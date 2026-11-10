//! IPC commands called by the front end, plus the pure helpers they rely on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;

use bf_core::{NodeId, NodeView, Rect, SortKey, Tree, TreemapOptions};
use bf_scan::{CancelToken, ScanError, ScanRoot, VolumeSummary};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::dto::{ScanDone, ScanErrorEvent, ScanProgressEvent};
use crate::state::AppState;

/// Largest page `get_children` returns.
const MAX_CHILDREN: u32 = 1000;

/// Parses `C`, `c:`, `C:\` or an existing folder path.
pub fn parse_target(arg: &str) -> Result<ScanRoot, String> {
    let arg = arg.trim();
    if arg.is_empty() {
        return Err("no scan target given".to_string());
    }
    if let Some(letter) = drive_letter(arg) {
        return Ok(ScanRoot::Volume(letter));
    }
    let path = Path::new(arg);
    if !path.exists() {
        return Err(format!("path does not exist: {arg}"));
    }
    if !path.is_dir() {
        return Err(format!("not a folder: {arg}"));
    }
    let absolute = std::path::absolute(path).map_err(|e| format!("{arg}: {e}"))?;
    Ok(ScanRoot::Folder(absolute))
}

/// The upper-case drive letter if `arg` is exactly a drive designator.
fn drive_letter(arg: &str) -> Option<char> {
    let mut chars = arg.chars();
    let letter = chars.next().filter(char::is_ascii_alphabetic)?;
    match chars.as_str() {
        "" | ":" | ":\\" | ":/" => Some(letter.to_ascii_uppercase()),
        _ => None,
    }
}

/// Converts a raw node number from the front end into a `NodeId` that exists in `tree`.
pub fn node_id_checked(tree: &Tree, node: u32) -> Result<NodeId, String> {
    if (node as usize) < tree.len() {
        Ok(NodeId(node))
    } else {
        Err("unknown node".to_string())
    }
}

/// Rejects canvas sizes that are not finite and positive.
pub fn validate_canvas(width: f32, height: f32) -> Result<(), String> {
    if width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0 {
        Ok(())
    } else {
        Err("invalid treemap size".to_string())
    }
}

/// Clones the current tree out of the state; the lock is released before returning.
fn current_tree(state: &AppState) -> Result<Arc<Tree>, String> {
    let guard = state
        .tree
        .read()
        .map_err(|_| "internal error: state poisoned")?;
    guard.clone().ok_or_else(|| "no scan yet".to_string())
}

/// Lists the mounted volumes.
#[tauri::command(async)]
pub fn list_drives() -> Vec<VolumeSummary> {
    bf_scan::list_volumes()
}

/// Whether the process runs with administrator rights.
#[tauri::command]
pub fn is_elevated() -> bool {
    bf_scan::is_elevated()
}

/// Starts a scan in the background and returns its id. Progress and the outcome arrive as
/// `scan://*` events tagged with that id. Any scan that is still running is cancelled first.
#[tauri::command(async)]
pub fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    target: String,
) -> Result<u64, String> {
    let root = parse_target(&target)?;
    let token = CancelToken::new();
    let scan_id = {
        let mut slot = state
            .scan
            .lock()
            .map_err(|_| "internal error: state poisoned")?;
        if let Some(previous) = slot.token.replace(token.clone()) {
            previous.cancel();
        }
        slot.id += 1;
        slot.id
    };
    tracing::info!("scan {scan_id} started: {root:?}");
    std::thread::spawn(move || run_scan(app, scan_id, root, token));
    Ok(scan_id)
}

/// Body of the scan thread: scans, stores the tree and reports the outcome.
fn run_scan(app: AppHandle, scan_id: u64, root: ScanRoot, token: CancelToken) {
    let progress_app = app.clone();
    let progress_token = token.clone();
    let progress = move |progress| {
        // A cancelled or superseded scan stays quiet.
        if !progress_token.is_cancelled() {
            let _ = progress_app.emit("scan://progress", ScanProgressEvent { scan_id, progress });
        }
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let result = bf_scan::scan_auto(&root, &progress, &token);
        finish_scan(&app, scan_id, &token, result)
    }));
    if outcome.is_err() {
        tracing::error!("scan {scan_id} panicked");
        report(&app, scan_id, |_| {
            Some(("internal error: scan crashed".to_string(), None))
        });
    }
}

/// Stores the tree and emits the outcome, but only while `scan_id` is still the current scan.
fn finish_scan(
    app: &AppHandle,
    scan_id: u64,
    token: &CancelToken,
    result: Result<(Tree, bf_scan::ScanStats), ScanError>,
) {
    match result {
        Ok((tree, stats)) if !token.is_cancelled() => {
            let tree = Arc::new(tree);
            let root = tree.view(tree.root());
            tracing::info!(
                "scan {scan_id} done: {} files, {} dirs",
                stats.files,
                stats.dirs
            );
            report(app, scan_id, |state| {
                match state.tree.write() {
                    Ok(mut slot) => *slot = Some(tree),
                    Err(_) => return Some(("internal error: state poisoned".to_string(), None)),
                }
                Some((
                    String::new(),
                    Some(ScanDone {
                        scan_id,
                        stats,
                        root,
                    }),
                ))
            });
        }
        Ok(_) | Err(ScanError::Cancelled) => {
            report(app, scan_id, |_| Some(("cancelled".to_string(), None)))
        }
        Err(e) => {
            tracing::warn!("scan {scan_id} failed: {e}");
            report(app, scan_id, |_| Some((e.to_string(), None)));
        }
    }
}

/// Runs `build` and emits its result while holding the scan lock, so a newer `start_scan`
/// cannot interleave. Does nothing if `scan_id` is no longer the current scan.
///
/// `build` returns an error message and, when the scan succeeded, the done payload.
fn report(
    app: &AppHandle,
    scan_id: u64,
    build: impl FnOnce(&AppState) -> Option<(String, Option<ScanDone>)>,
) {
    let state = app.state::<AppState>();
    // A poisoned lock only means an earlier panic; the data is still usable.
    let slot = state
        .scan
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot.id != scan_id {
        return;
    }
    match build(&state) {
        Some((_, Some(done))) => {
            let _ = app.emit("scan://done", done);
        }
        Some((message, None)) => {
            let _ = app.emit("scan://error", ScanErrorEvent { scan_id, message });
        }
        None => {}
    }
}

/// Cancels the running scan, if any.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) -> Result<(), String> {
    let slot = state
        .scan
        .lock()
        .map_err(|_| "internal error: state poisoned")?;
    if let Some(token) = slot.token.as_ref() {
        token.cancel();
    }
    Ok(())
}

/// One page of a directory's children, sorted by `sort` (`limit` is capped at 1000).
#[tauri::command(async)]
pub fn get_children(
    state: State<'_, AppState>,
    node: u32,
    sort: SortKey,
    offset: u32,
    limit: u32,
) -> Result<Vec<NodeView>, String> {
    let tree = current_tree(&state)?;
    let id = node_id_checked(&tree, node)?;
    Ok(tree.children(id, sort, offset as usize, limit.min(MAX_CHILDREN) as usize))
}

/// Details of one node.
#[tauri::command]
pub fn get_node(state: State<'_, AppState>, node: u32) -> Result<NodeView, String> {
    let tree = current_tree(&state)?;
    let id = node_id_checked(&tree, node)?;
    Ok(tree.view(id))
}

/// Full path of a node.
#[tauri::command]
pub fn get_path(state: State<'_, AppState>, node: u32) -> Result<String, String> {
    let tree = current_tree(&state)?;
    let id = node_id_checked(&tree, node)?;
    Ok(tree.path_of(id))
}

/// Treemap rectangles (at most 20 000) of the subtree under `node`, in a `width` x `height` canvas.
#[tauri::command(async)]
pub fn get_treemap(
    state: State<'_, AppState>,
    node: u32,
    width: f32,
    height: f32,
) -> Result<Vec<Rect>, String> {
    validate_canvas(width, height)?;
    let tree = current_tree(&state)?;
    let id = node_id_checked(&tree, node)?;
    let opts = TreemapOptions {
        width,
        height,
        ..Default::default()
    };
    Ok(tree.treemap(id, &opts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bf_core::{EntryRecord, NodeFlags, TreeBuilder};

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

    #[test]
    fn drive_forms_become_volumes() {
        for arg in ["C:", "c:", "C:\\", "c:\\", "C", "c"] {
            assert!(
                matches!(parse_target(arg), Ok(ScanRoot::Volume('C'))),
                "{arg}"
            );
        }
    }

    #[test]
    fn existing_folder_becomes_absolute_folder() {
        match parse_target(std::env::temp_dir().to_str().unwrap()) {
            Ok(ScanRoot::Folder(p)) => assert!(p.is_absolute()),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn empty_or_missing_target_is_an_error() {
        assert!(parse_target("").is_err());
        assert!(parse_target("   ").is_err());
        assert!(parse_target(r"E:\no\such\folder\here").is_err());
    }

    #[test]
    fn a_file_is_not_a_scan_target() {
        let file = std::env::temp_dir().join(format!("bf-app-test-{}.txt", std::process::id()));
        std::fs::write(&file, b"x").unwrap();
        let err = parse_target(file.to_str().unwrap());
        std::fs::remove_file(&file).unwrap();
        assert!(err.unwrap_err().starts_with("not a folder"));
    }

    #[test]
    fn node_ids_are_range_checked() {
        let tree = tiny_tree();
        assert_eq!(node_id_checked(&tree, 0), Ok(NodeId(0)));
        assert_eq!(node_id_checked(&tree, 1), Ok(NodeId(1)));
        assert_eq!(
            node_id_checked(&tree, 999_999),
            Err("unknown node".to_string())
        );
        assert!(node_id_checked(&tree, u32::MAX).is_err());
    }

    #[test]
    fn canvas_must_be_finite_and_positive() {
        assert!(validate_canvas(800.0, 600.0).is_ok());
        for (w, h) in [
            (0.0, 10.0),
            (10.0, 0.0),
            (-1.0, 10.0),
            (f32::NAN, 10.0),
            (10.0, f32::INFINITY),
        ] {
            assert!(validate_canvas(w, h).is_err(), "{w} x {h}");
        }
    }
}
