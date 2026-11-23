//! Automatic snapshots: every finished live scan is saved in the background to
//! `<app local data>/snapshots/auto/`, the newest N are kept, and a saved snapshot can be
//! reopened instantly, compared with the current scan or deleted.
//!
//! The front end only ever names a snapshot by its bare file name (`id`). Ids are validated
//! by [`is_valid_snapshot_id`] and resolved inside the auto folder by [`resolve_id`], so a
//! crafted id can neither leave that folder nor reach a file that is not a `.bfsnap`.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use bf_core::{NodeView, Tree, read_snapshot_header};
use bf_scan::CancelToken;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::commands::{
    current_source, current_tree, drop_baseline, file_name_part_ascii, install_baseline,
    load_baseline, read_snapshot_file, unix_ms_now, utc_stamp, write_snapshot_file,
};
use crate::dto::{BaselineInfo, SourceDto};
use crate::state::{AppState, CompletedScan, Source};

/// Event sent after the list of automatic snapshots changed.
pub const SNAPSHOTS_CHANGED: &str = "snapshots-changed";

const EXTENSION: &str = ".bfsnap";
/// Longest id stem (the file name without `.bfsnap`).
const MAX_STEM_LEN: usize = 200;

/// Serialises automatic saves so two finishing scans cannot prune each other's file.
static AUTO_LOCK: Mutex<()> = Mutex::new(());

/// One saved automatic snapshot as listed in the sidebar.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct AutoSnapshot {
    /// File name inside the auto folder; the handle every command takes.
    pub id: String,
    /// What was scanned (`C:` or a folder path).
    pub label: String,
    pub created_unix_ms: i64,
    pub node_count: u32,
    pub bytes_on_disk: u64,
}

/// Result of `open_snapshot`: like `scan://done`, for a result loaded from a file.
#[derive(Serialize, Clone, Debug)]
pub struct OpenedSnapshot {
    pub scan_id: u64,
    /// What was scanned when the snapshot was taken.
    pub target: String,
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub root: NodeView,
    pub source: SourceDto,
}

// ---------------------------------------------------------------- pure pieces

/// Whether `id` is a bare snapshot file name: `^[A-Za-z0-9._-]{1,200}\.bfsnap$` with no `..`,
/// no leading dot and no Windows device name (`CON.bfsnap`) as its stem.
pub fn is_valid_snapshot_id(id: &str) -> bool {
    let Some(stem) = id.strip_suffix(EXTENSION) else {
        return false;
    };
    if stem.is_empty() || stem.len() > MAX_STEM_LEN || id.contains("..") {
        return false;
    }
    if !stem
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return false;
    }
    if stem.starts_with('.') {
        return false;
    }
    !is_reserved_device_name(stem.split('.').next().unwrap_or(stem))
}

/// Windows treats these names as devices whatever the extension.
fn is_reserved_device_name(base: &str) -> bool {
    let upper = base.to_ascii_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    ["COM", "LPT"].iter().any(|prefix| {
        upper
            .strip_prefix(prefix)
            .is_some_and(|n| n.len() == 1 && n.as_bytes()[0].is_ascii_digit())
    })
}

/// `<target>-<YYYYMMDD-HHMMSS>.bfsnap`, always a valid id.
pub fn auto_snapshot_name(target: &str, unix_ms: i64) -> String {
    let secs = unix_ms.div_euclid(1000).rem_euclid(60);
    format!(
        "{}-{}{secs:02}.bfsnap",
        file_name_part_ascii(target),
        utc_stamp(unix_ms)
    )
}

/// Ids to delete so that only the newest `keep` remain. Entries are `(id, created_unix_ms)`;
/// ties on the time are broken by id (the greater id counts as newer), so the result does
/// not depend on the input order.
pub fn snapshots_to_delete<I: Ord + Clone>(entries: &[(I, i64)], keep: usize) -> Vec<I> {
    let mut sorted: Vec<&(I, i64)> = entries.iter().collect();
    sorted.sort_by(|a, b| (b.1, &b.0).cmp(&(a.1, &a.0)));
    sorted
        .into_iter()
        .skip(keep)
        .map(|(id, _)| id.clone())
        .collect()
}

// ---------------------------------------------------------------- files

/// `<app local data>/snapshots/auto`.
fn auto_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map(|dir| dir.join("snapshots").join("auto"))
        .map_err(|e| format!("The app data folder is not available: {e}"))
}

/// The file of snapshot `id` inside `dir`, after checking that it is a plain file whose
/// canonical parent is `dir` itself (symbolic links and traversal tricks are refused).
pub fn resolve_id(dir: &Path, id: &str) -> Result<PathBuf, String> {
    if !is_valid_snapshot_id(id) {
        return Err("Invalid snapshot.".to_owned());
    }
    let missing = || "This snapshot no longer exists.".to_owned();
    let dir = dir.canonicalize().map_err(|_| missing())?;
    let path = dir.join(id);
    let meta = std::fs::symlink_metadata(&path).map_err(|_| missing())?;
    if !meta.is_file() {
        return Err(missing());
    }
    let canonical = path.canonicalize().map_err(|_| missing())?;
    if canonical.parent() != Some(dir.as_path()) {
        return Err("Invalid snapshot.".to_owned());
    }
    Ok(canonical)
}

/// The readable snapshots in `dir`, newest first. Files with other names, folders, links and
/// files whose header cannot be read are skipped.
pub fn list_dir(dir: &Path) -> Vec<AutoSnapshot> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in read.flatten() {
        let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_valid_snapshot_id(&id) {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let Ok(header) = read_snapshot_header(file) else {
            continue;
        };
        found.push(AutoSnapshot {
            id,
            label: header.label,
            created_unix_ms: header.created_unix_ms,
            node_count: header.node_count,
            bytes_on_disk: meta.len(),
        });
    }
    found.sort_by(|a, b| (b.created_unix_ms, &b.id).cmp(&(a.created_unix_ms, &a.id)));
    found
}

/// Deletes all but the newest `keep` snapshots of `dir` (across every target) and returns
/// the ids removed. Only files [`list_dir`] accepts are ever touched.
pub fn apply_retention(dir: &Path, keep: usize) -> Vec<String> {
    let entries: Vec<(String, i64)> = list_dir(dir)
        .into_iter()
        .map(|s| (s.id, s.created_unix_ms))
        .collect();
    let mut removed = Vec::new();
    for id in snapshots_to_delete(&entries, keep) {
        match std::fs::remove_file(dir.join(&id)) {
            Ok(()) => removed.push(id),
            Err(e) => tracing::warn!("cannot remove old snapshot {id}: {e}"),
        }
    }
    removed
}

/// Writes the snapshot of a finished scan into `dir` and prunes `dir`.
fn save_into(dir: &Path, tree: &Tree, target: &str, keep: usize) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let created = unix_ms_now();
    let path = dir.join(auto_snapshot_name(target, created));
    write_snapshot_file(tree, target, created, &path)?;
    apply_retention(dir, keep);
    Ok(())
}

/// Saves `tree` in the background and tells the front end when done. Never blocks the caller
/// and never reports a failure to the user; it is only logged.
pub fn spawn_auto_save(app: AppHandle, tree: Arc<Tree>, target: String, keep: usize) {
    std::thread::spawn(move || {
        let _guard = AUTO_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let result = auto_dir(&app).and_then(|dir| save_into(&dir, &tree, &target, keep));
        match result {
            Ok(()) => {
                let _ = app.emit(SNAPSHOTS_CHANGED, ());
            }
            Err(e) => tracing::warn!("automatic snapshot failed: {e}"),
        }
    });
}

// ---------------------------------------------------------------- commands

/// The saved automatic snapshots, newest first.
#[tauri::command]
pub async fn list_auto_snapshots(app: AppHandle) -> Result<Vec<AutoSnapshot>, String> {
    let dir = auto_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || list_dir(&dir))
        .await
        .map_err(|e| e.to_string())
}

/// Replaces the current result with the snapshot `id`, read-only. Refused while a scan runs.
#[tauri::command]
pub async fn open_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<OpenedSnapshot, String> {
    let started_at = {
        let slot = state
            .scan
            .lock()
            .map_err(|_| "internal error: state poisoned")?;
        refuse_while_scanning(slot.running, slot.token.as_ref())?;
        slot.id
    };
    let dir = auto_dir(&app)?;
    let snap =
        tauri::async_runtime::spawn_blocking(move || read_snapshot_file(&resolve_id(&dir, &id)?))
            .await
            .map_err(|e| e.to_string())??;

    let created_unix_ms = snap.created_unix_ms;
    let target = snap.label;
    let tree = Arc::new(snap.tree);
    let root = tree.view(tree.root());

    // Lock order: scan, tree, baseline.
    let mut slot = state
        .scan
        .lock()
        .map_err(|_| "internal error: state poisoned")?;
    refuse_while_scanning(slot.running, slot.token.as_ref())?;
    if slot.id != started_at {
        return Err("A scan ran while the snapshot was loading.".to_owned());
    }
    if let Some(token) = slot.token.take() {
        token.cancel();
    }
    slot.id += 1;
    slot.running = false;
    let scan_id = slot.id;
    *state
        .tree
        .write()
        .map_err(|_| "internal error: state poisoned")? = Some(CompletedScan {
        id: scan_id,
        tree,
        target: target.clone(),
        source: Source::Snapshot { created_unix_ms },
    });
    drop_baseline(&state);
    drop(slot);
    Ok(OpenedSnapshot {
        scan_id,
        target,
        files: root.files,
        dirs: root.dirs,
        bytes: root.size,
        root,
        source: SourceDto::Snapshot { created_unix_ms },
    })
}

/// A running scan (not yet cancelled) blocks opening a snapshot.
fn refuse_while_scanning(running: bool, token: Option<&CancelToken>) -> Result<(), String> {
    if running && !token.is_some_and(CancelToken::is_cancelled) {
        return Err("A scan is running. Stop it or wait for it to finish first.".to_owned());
    }
    Ok(())
}

/// Compares the live result `scan_id` with the saved snapshot `id`.
#[tauri::command]
pub async fn compare_with_auto_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
    id: String,
) -> Result<BaselineInfo, String> {
    let tree = current_tree(&state, Some(scan_id))?;
    if current_source(&state)? != Source::Live {
        return Err("Comparing needs a live scan; rescan first.".to_owned());
    }
    let dir = auto_dir(&app)?;
    let compared = Arc::clone(&tree);
    let loaded = tauri::async_runtime::spawn_blocking(move || {
        load_baseline(&resolve_id(&dir, &id)?, &compared)
    })
    .await
    .map_err(|e| e.to_string())??;
    install_baseline(&state, scan_id, &tree, loaded)
}

/// Deletes the saved snapshot `id` (that file only).
#[tauri::command]
pub async fn delete_auto_snapshot(app: AppHandle, id: String) -> Result<(), String> {
    let dir = auto_dir(&app)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = resolve_id(&dir, &id)?;
        std::fs::remove_file(path).map_err(|e| format!("Could not delete the snapshot: {e}"))
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.emit(SNAPSHOTS_CHANGED, ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use bf_core::{EntryRecord, NodeFlags, TreeBuilder};

    use super::*;

    fn tiny_tree() -> Tree {
        let mut b = TreeBuilder::new(5, "C:");
        b.push(EntryRecord {
            key: 6,
            parent_key: 5,
            name: "a.txt".into(),
            flags: NodeFlags::empty(),
            size: 10,
            alloc: 4096,
            mtime: 0,
        });
        b.finish()
    }

    fn temp_dir() -> tempfile::TempDir {
        match std::env::var_os("BF_TEST_TMP") {
            Some(base) => tempfile::tempdir_in(base).unwrap(),
            None => tempfile::tempdir().unwrap(),
        }
    }

    fn write_at(dir: &Path, name: &str, created: i64) {
        write_snapshot_file(&tiny_tree(), "C:", created, &dir.join(name)).unwrap();
    }

    #[test]
    fn plain_bfsnap_names_are_valid_ids() {
        for id in [
            "C-20260102-030405.bfsnap",
            "a.bfsnap",
            "my_folder-1.2.bfsnap",
            "Z.BFSNAP.bfsnap",
        ] {
            assert!(is_valid_snapshot_id(id), "{id}");
        }
        assert!(is_valid_snapshot_id(&format!("{}.bfsnap", "a".repeat(200))));
    }

    #[test]
    fn ids_with_separators_traversal_or_wrong_shape_are_refused() {
        for id in [
            "",
            ".bfsnap",
            "a.txt",
            "a.bfsnap.tmp",
            "a.bfsnap ",
            "..\\a.bfsnap",
            "../a.bfsnap",
            "sub/a.bfsnap",
            "sub\\a.bfsnap",
            "C:a.bfsnap",
            "C:\\a.bfsnap",
            "a..b.bfsnap",
            "..bfsnap",
            "...bfsnap",
            ".hidden.bfsnap",
            "a\0.bfsnap",
            "\u{e9}.bfsnap",
            "a b.bfsnap",
            "CON.bfsnap",
            "nul.bfsnap",
            "com1.bfsnap",
            "LPT9.x.bfsnap",
        ] {
            assert!(!is_valid_snapshot_id(id), "{id:?}");
        }
        assert!(!is_valid_snapshot_id(&format!(
            "{}.bfsnap",
            "a".repeat(201)
        )));
    }

    #[test]
    fn generated_names_are_valid_ids_whatever_the_target() {
        let ms = 1_700_000_000_123;
        for target in [
            "C:",
            r"C:\Users\me\My Documents",
            "/Users/me/\u{43f}\u{440}\u{438}\u{432}\u{435}\u{442}",
            "",
            "CON",
            "....",
            &"x".repeat(500),
        ] {
            let name = auto_snapshot_name(target, ms);
            assert!(is_valid_snapshot_id(&name), "{target:?} -> {name}");
        }
        assert_eq!(auto_snapshot_name("C:", ms), "C-20231114-221320.bfsnap");
    }

    #[test]
    fn retention_keeps_the_newest_and_breaks_ties_by_id() {
        let entries = [
            ("a".to_string(), 10),
            ("b".to_string(), 30),
            ("c".to_string(), 20),
            ("d".to_string(), 30),
        ];
        let mut gone = snapshots_to_delete(&entries, 2);
        gone.sort();
        // Newest are d and b (tie on 30, greater id first); a and c go.
        assert_eq!(gone, ["a", "c"]);
        assert!(snapshots_to_delete(&entries, 4).is_empty());
        assert!(snapshots_to_delete(&entries, 99).is_empty());
        assert_eq!(snapshots_to_delete(&entries, 0).len(), 4);
        // Tie at the cut: the smaller id is dropped, whatever the input order.
        let tied = [("x".to_string(), 5), ("y".to_string(), 5)];
        let reversed = [("y".to_string(), 5), ("x".to_string(), 5)];
        assert_eq!(snapshots_to_delete(&tied, 1), ["x"]);
        assert_eq!(snapshots_to_delete(&reversed, 1), ["x"]);
        assert!(snapshots_to_delete::<String>(&[], 3).is_empty());
    }

    #[test]
    fn listing_is_newest_first_and_skips_foreign_and_broken_files() {
        let dir = temp_dir();
        write_at(dir.path(), "old-1.bfsnap", 100);
        write_at(dir.path(), "new-1.bfsnap", 300);
        write_at(dir.path(), "mid-1.bfsnap", 200);
        std::fs::write(dir.path().join("broken.bfsnap"), b"not a snapshot").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"hello").unwrap();
        std::fs::write(dir.path().join("half.bfsnap.tmp"), b"BFSNAP").unwrap();
        std::fs::create_dir(dir.path().join("folder.bfsnap")).unwrap();
        let listed = list_dir(dir.path());
        let ids: Vec<&str> = listed.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["new-1.bfsnap", "mid-1.bfsnap", "old-1.bfsnap"]);
        assert_eq!(listed[0].label, "C:");
        assert_eq!(listed[0].created_unix_ms, 300);
        assert_eq!(listed[0].node_count, 2);
        assert!(listed[0].bytes_on_disk > 0);
        assert!(list_dir(&dir.path().join("missing")).is_empty());
    }

    #[test]
    fn retention_removes_old_snapshots_and_leaves_other_files_alone() {
        let dir = temp_dir();
        for (i, name) in ["a-1", "b-2", "c-3", "d-4"].iter().enumerate() {
            write_at(dir.path(), &format!("{name}.bfsnap"), i as i64 * 10);
        }
        std::fs::write(dir.path().join("broken.bfsnap"), b"x").unwrap();
        std::fs::write(dir.path().join("keep.txt"), b"x").unwrap();
        let removed = apply_retention(dir.path(), 2);
        assert_eq!(removed.len(), 2);
        let left: Vec<String> = list_dir(dir.path()).into_iter().map(|s| s.id).collect();
        assert_eq!(left, ["d-4.bfsnap", "c-3.bfsnap"]);
        assert!(dir.path().join("broken.bfsnap").exists());
        assert!(dir.path().join("keep.txt").exists());
    }

    #[test]
    fn save_writes_a_readable_snapshot_without_temp_file_and_prunes() {
        let dir = temp_dir();
        let auto = dir.path().join("snapshots").join("auto");
        write_snapshot_file(&tiny_tree(), "C:", 1, &{
            std::fs::create_dir_all(&auto).unwrap();
            auto.join("ancient-1.bfsnap")
        })
        .unwrap();
        save_into(&auto, &tiny_tree(), "C:", 1).unwrap();
        let listed = list_dir(&auto);
        assert_eq!(listed.len(), 1);
        assert_ne!(listed[0].id, "ancient-1.bfsnap");
        assert_eq!(listed[0].label, "C:");
        let names: Vec<String> = std::fs::read_dir(&auto)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.iter().all(|n| !n.ends_with(".tmp")), "{names:?}");
    }

    #[test]
    fn resolving_finds_plain_files_and_refuses_everything_else() {
        let dir = temp_dir();
        let auto = dir.path().join("auto");
        std::fs::create_dir_all(&auto).unwrap();
        write_at(&auto, "ok-1.bfsnap", 1);
        // A snapshot next to the folder must stay out of reach.
        write_at(dir.path(), "outside.bfsnap", 1);
        std::fs::create_dir(auto.join("dir.bfsnap")).unwrap();

        let path = resolve_id(&auto, "ok-1.bfsnap").unwrap();
        assert_eq!(path.file_name().unwrap(), "ok-1.bfsnap");
        for id in [
            "../outside.bfsnap",
            "..\\outside.bfsnap",
            "outside.bfsnap",
            "missing.bfsnap",
            "dir.bfsnap",
            "ok-1.txt",
            "",
        ] {
            assert!(resolve_id(&auto, id).is_err(), "{id:?}");
        }
        assert!(resolve_id(&dir.path().join("nope"), "ok-1.bfsnap").is_err());
    }

    #[test]
    fn a_scan_that_is_running_blocks_opening_but_a_cancelled_one_does_not() {
        let token = CancelToken::new();
        assert!(refuse_while_scanning(false, None).is_ok());
        assert!(refuse_while_scanning(true, Some(&token)).is_err());
        token.cancel();
        assert!(refuse_while_scanning(true, Some(&token)).is_ok());
    }

    #[test]
    fn snapshot_results_refuse_trashing_but_live_results_do_not() {
        use crate::commands::SNAPSHOT_READ_ONLY;
        let state = AppState::default();
        let set = |source| {
            *state.tree.write().unwrap() = Some(CompletedScan {
                id: 3,
                tree: Arc::new(tiny_tree()),
                target: "C:".into(),
                source,
            });
            state.scan.lock().unwrap().id = 3;
        };
        set(Source::Snapshot { created_unix_ms: 9 });
        assert_eq!(
            current_source(&state),
            Ok(Source::Snapshot { created_unix_ms: 9 })
        );
        let denied = crate::commands::trash_target_live(&state, 3, 1);
        assert_eq!(denied, Err(SNAPSHOT_READ_ONLY.to_owned()));
        set(Source::Live);
        // Live results get past the guard (the path itself may still be refused).
        assert_ne!(
            crate::commands::trash_target_live(&state, 3, 1),
            Err(SNAPSHOT_READ_ONLY.to_owned())
        );
    }
}
