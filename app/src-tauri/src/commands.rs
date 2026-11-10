//! IPC commands called by the front end, plus the pure helpers they rely on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;

use bf_core::{NodeId, NodeView, Rect, SortKey, Tree, TreemapOptions};
use bf_scan::{CancelToken, ScanError, ScanRoot, VolumeSummary};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::dto::{ScanDone, ScanErrorEvent};
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
#[tauri::command]
pub fn list_drives() -> Vec<VolumeSummary> {
    bf_scan::list_volumes()
}

/// Whether the process runs with administrator rights.
#[tauri::command]
pub fn is_elevated() -> bool {
    bf_scan::is_elevated()
}

/// Starts a scan in the background; progress and the outcome arrive as `scan://*` events.
/// Any scan that is still running is cancelled first.
#[tauri::command]
pub fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    target: String,
) -> Result<(), String> {
    let root = parse_target(&target)?;
    let token = CancelToken::new();
    {
        let mut slot = state
            .cancel
            .lock()
            .map_err(|_| "internal error: state poisoned")?;
        if let Some(previous) = slot.replace(token.clone()) {
            previous.cancel();
        }
    }
    tracing::info!("scan started: {root:?}");
    std::thread::spawn(move || run_scan(app, root, token));
    Ok(())
}

/// Body of the scan thread: scans, stores the tree and reports the outcome.
fn run_scan(app: AppHandle, root: ScanRoot, token: CancelToken) {
    let progress_app = app.clone();
    let progress = move |p| {
        let _ = progress_app.emit("scan://progress", p);
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        bf_scan::scan_auto(&root, &progress, &token)
    }));

    match outcome {
        Ok(Ok((tree, stats))) => {
            // A newer scan may have cancelled this one just as it finished; drop the result.
            if token.is_cancelled() {
                emit_error(&app, "cancelled");
                return;
            }
            let tree = Arc::new(tree);
            let root_view = tree.view(tree.root());
            let state = app.state::<AppState>();
            match state.tree.write() {
                Ok(mut slot) => *slot = Some(tree),
                Err(_) => return emit_error(&app, "internal error: state poisoned"),
            }
            tracing::info!("scan done: {} files, {} dirs", stats.files, stats.dirs);
            let _ = app.emit(
                "scan://done",
                ScanDone {
                    stats,
                    root: root_view,
                },
            );
        }
        Ok(Err(ScanError::Cancelled)) => emit_error(&app, "cancelled"),
        Ok(Err(e)) => {
            tracing::warn!("scan failed: {e}");
            emit_error(&app, &e.to_string());
        }
        Err(_) => {
            tracing::error!("scan thread panicked");
            emit_error(&app, "internal error: scan crashed");
        }
    }
}

fn emit_error(app: &AppHandle, message: &str) {
    let _ = app.emit(
        "scan://error",
        ScanErrorEvent {
            message: message.to_string(),
        },
    );
}

/// Cancels the running scan, if any.
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>) -> Result<(), String> {
    let slot = state
        .cancel
        .lock()
        .map_err(|_| "internal error: state poisoned")?;
    if let Some(token) = slot.as_ref() {
        token.cancel();
    }
    Ok(())
}

/// One page of a directory's children, sorted by `sort` (`limit` is capped at 1000).
#[tauri::command]
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
#[tauri::command]
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
        match parse_target(r"E:\Projects") {
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
