//! Integration tests for `WalkScanner`, `list_volumes` and `is_elevated`.
//!
//! Temp directories live under `D:\Temp` when it exists, else in the system temp dir.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;

use bf_core::{NodeFlags, NodeId, Tree};
use bf_scan::{
    CancelToken, Phase, ScanError, ScanMethod, ScanProgress, ScanRoot, ScanStats, Scanner,
    WalkScanner, list_volumes,
};
use tempfile::TempDir;

fn temp_dir() -> TempDir {
    let preferred = Path::new(r"D:\Temp");
    if preferred.is_dir() {
        tempfile::tempdir_in(preferred).expect("create temp dir under D:\\Temp")
    } else {
        tempfile::tempdir().expect("create temp dir")
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
struct RestoreAcl<'a>(&'a Path);

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
