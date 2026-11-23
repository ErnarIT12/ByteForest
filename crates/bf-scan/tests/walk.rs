//! Integration tests for `WalkScanner`, `list_volumes` and `is_elevated`.
//!
//! Temp directories live under `BF_TEST_TMP` when set, else in the system temp dir.

use std::fs;
use std::path::Path;
#[cfg(windows)]
use std::process::Command;
use std::sync::Mutex;

use bf_core::{NodeFlags, NodeId, Tree};
#[cfg(windows)]
use bf_scan::list_volumes;
use bf_scan::{
    CancelToken, Phase, ScanError, ScanMethod, ScanProgress, ScanRoot, ScanStats, Scanner,
    WalkScanner,
};
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    match std::env::var_os("BF_TEST_TMP") {
        Some(base) => tempfile::tempdir_in(base).expect("create temp dir under BF_TEST_TMP"),
        None => tempfile::tempdir().expect("create temp dir"),
    }
}

fn scan_folder(path: &Path) -> Result<(Tree, ScanStats), ScanError> {
    WalkScanner.scan(
        &ScanRoot::Folder(path.to_path_buf()),
        &|_| {},
        &CancelToken::default(),
    )
}

fn child_named(tree: &Tree, parent: NodeId, name: &str) -> Option<NodeId> {
    tree.children_ids(parent).find(|&c| tree.name(c) == name)
}

#[test]
fn walk_counts_files_and_sizes() {
    let tmp = temp_dir();
    let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::write(a.join("f10.bin"), [0u8; 10]).unwrap();
    fs::write(a.join("f20.bin"), [0u8; 20]).unwrap();
    fs::write(b.join("f30.bin"), [0u8; 30]).unwrap();

    let (tree, stats) = scan_folder(tmp.path()).unwrap();

    let total = tree.total(tree.root());
    assert_eq!(total.size, 60);
    assert_eq!(total.files, 3);
    assert_eq!(total.dirs, 2);
    assert!(total.alloc >= total.size, "alloc is rounded up to clusters");
    assert_eq!(tree.name(tree.root()), tmp.path().display().to_string());

    assert_eq!((stats.files, stats.dirs, stats.bytes), (3, 2, 60));
    assert_eq!(stats.method, ScanMethod::Walk);
    assert_eq!(stats.errors, 0);
    assert_eq!(stats.skipped_records, 0);
    assert!(stats.fallback_reason.is_none());

    let a_id = child_named(&tree, tree.root(), "a").expect("dir a");
    assert!(tree.flags(a_id).contains(NodeFlags::DIR));
    assert_eq!(tree.total(a_id).size, 30);
}

#[test]
#[cfg(windows)]
fn walk_does_not_follow_junction_loop() {
    let tmp = temp_dir();
    fs::write(tmp.path().join("real.bin"), [0u8; 5]).unwrap();
    let link = tmp.path().join("loop");
    let status = Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(&link)
        .arg(tmp.path())
        .output()
        .expect("cmd.exe must be runnable");
    assert!(
        status.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&status.stderr)
    );

    let (tree, stats) = scan_folder(tmp.path()).unwrap();

    let node = child_named(&tree, tree.root(), "loop").expect("junction node");
    assert!(tree.flags(node).contains(NodeFlags::REPARSE));
    assert_eq!(tree.total(node).size, 0);
    assert_eq!(
        tree.children_ids(node).count(),
        0,
        "junction is not entered"
    );
    assert_eq!((stats.files, stats.bytes), (1, 5));
}

/// Removes the deny ACE on drop so the temp dir can be deleted even if an assertion fails.
#[cfg(windows)]
struct RestoreAcl<'a>(&'a Path);

#[cfg(windows)]
impl Drop for RestoreAcl<'_> {
    fn drop(&mut self) {
        let _ = Command::new("icacls")
            .arg(self.0)
            .arg("/remove:d")
            .arg("*S-1-1-0")
            .output();
    }
}

#[test]
#[cfg(windows)]
fn walk_continues_after_access_denied() {
    let tmp = temp_dir();
    let denied = tmp.path().join("denied");
    fs::create_dir(&denied).unwrap();
    fs::write(tmp.path().join("ok.bin"), [0u8; 7]).unwrap();

    let guard = RestoreAcl(&denied);
    let out = Command::new("icacls")
        .arg(&denied)
        .arg("/deny")
        .arg("*S-1-1-0:(OI)(CI)(RX)")
        .output()
        .expect("icacls must be runnable");
    assert!(
        out.status.success(),
        "icacls /deny failed: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let result = scan_folder(tmp.path());
    drop(guard);

    let (tree, stats) = result.unwrap();
    assert!(stats.errors >= 1, "errors = {}", stats.errors);
    assert_eq!(stats.files, 1);
    assert_eq!(stats.bytes, 7);
    assert!(child_named(&tree, tree.root(), "ok.bin").is_some());
}

#[test]
fn walk_cancel_returns_cancelled() {
    let tmp = temp_dir();
    fs::write(tmp.path().join("x.bin"), [0u8; 1]).unwrap();
    let cancel = CancelToken::default();
    cancel.cancel();

    let result = WalkScanner.scan(
        &ScanRoot::Folder(tmp.path().to_path_buf()),
        &|_| {},
        &cancel,
    );

    assert!(matches!(result, Err(ScanError::Cancelled)));
}

#[test]
fn walk_missing_root_is_an_error() {
    let tmp = temp_dir();
    let result = scan_folder(&tmp.path().join("does-not-exist"));
    assert!(matches!(result, Err(ScanError::Io(_))));
}

#[test]
fn walk_reports_final_progress() {
    let tmp = temp_dir();
    fs::create_dir(tmp.path().join("d")).unwrap();
    fs::write(tmp.path().join("d").join("a.bin"), [0u8; 12]).unwrap();
    fs::write(tmp.path().join("b.bin"), [0u8; 8]).unwrap();

    let seen: Mutex<Vec<ScanProgress>> = Mutex::new(Vec::new());
    let (_, stats) = WalkScanner
        .scan(
            &ScanRoot::Folder(tmp.path().to_path_buf()),
            &|p| seen.lock().unwrap().push(p),
            &CancelToken::default(),
        )
        .unwrap();

    let seen = seen.into_inner().unwrap();
    let last = seen.last().expect("at least one progress call");
    assert_eq!(last.phase, Phase::Building);
    assert_eq!((last.files, last.dirs, last.bytes), (2, 1, 20));
    assert_eq!(stats.bytes, 20);
}

#[test]
#[cfg(windows)]
fn list_volumes_contains_system_drive() {
    let system_drive = std::env::var("SystemDrive").expect("SystemDrive is set on Windows");
    let letter = system_drive
        .chars()
        .next()
        .expect("SystemDrive is not empty")
        .to_ascii_uppercase();

    let volumes = list_volumes();

    let vol = volumes
        .iter()
        .find(|v| v.letter == letter)
        .unwrap_or_else(|| panic!("{letter}: not in {volumes:?}"));
    assert_eq!(vol.fs, "NTFS");
    assert!(vol.total > 0 && vol.free <= vol.total);
}

#[test]
#[cfg(target_os = "macos")]
fn mac_walk_does_not_follow_symlinks_and_reads_file_metadata() {
    use std::os::unix::fs::{MetadataExt, symlink};

    let root = temp_dir();
    let outside = temp_dir();
    fs::write(outside.path().join("outside.bin"), [0u8; 20]).unwrap();
    symlink(outside.path(), root.path().join("link")).unwrap();
    let file = root.path().join("name with spaces.bin");
    fs::write(&file, [0u8; 7]).unwrap();

    let (tree, stats) = scan_folder(root.path()).unwrap();
    let link = child_named(&tree, tree.root(), "link").unwrap();
    let node = child_named(&tree, tree.root(), "name with spaces.bin").unwrap();
    assert!(tree.flags(link).contains(NodeFlags::REPARSE));
    assert_eq!(tree.children_ids(link).count(), 0);
    assert_eq!(tree.total(link).size, 0);
    assert_eq!(
        tree.total(node).alloc,
        fs::metadata(file).unwrap().blocks() * 512
    );
    assert!(tree.mtime(node) > 116_444_736_000_000_000);
    assert_eq!(stats.bytes, 7);
}

#[test]
#[cfg(target_os = "macos")]
fn mac_walk_keeps_results_when_a_folder_cannot_be_read() {
    use std::os::unix::fs::PermissionsExt;

    let root = temp_dir();
    let denied = root.path().join("no access");
    fs::create_dir(&denied).unwrap();
    fs::write(denied.join("invisible"), b"x").unwrap();
    fs::write(root.path().join("visible"), b"visible").unwrap();
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).unwrap();
    let permission_enforced = fs::read_dir(&denied).is_err();
    let result = scan_folder(root.path());
    fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();

    let (tree, stats) = result.unwrap();
    assert!(child_named(&tree, tree.root(), "visible").is_some());
    if permission_enforced {
        assert_eq!(stats.errors, 1);
        assert_eq!(stats.bytes, 7);
    }
}
