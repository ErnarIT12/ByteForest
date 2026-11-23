//! IPC commands called by the front end, plus the pure helpers they rely on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use bf_core::{FileFilter, NodeFlags, NodeId, NodeView, Rect, SortKey, Tree, TreemapOptions};
use bf_scan::{CancelToken, ScanError, ScanRoot, VolumeSummary};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::dto::{ScanDone, ScanErrorEvent, ScanProgressEvent, TreemapRect};
#[cfg(windows)]
use crate::elevation::{self, ElevateError};
use crate::settings::{self as store, Settings};
use crate::state::{AppState, CompletedScan};
use tauri_plugin_clipboard_manager::ClipboardExt;

/// Largest page `get_children` returns.
const MAX_CHILDREN: u32 = 1000;

/// Tallest directory name strip `get_treemap` accepts, in pixels.
const MAX_HEADER: f32 = 128.0;

/// Parses `C`, `c:`, `C:\` or an existing folder path.
pub fn parse_target(arg: &str) -> Result<ScanRoot, String> {
    #[cfg(windows)]
    let arg = arg.trim();
    if arg.trim().is_empty() {
        return Err("no scan target given".to_string());
    }
    #[cfg(windows)]
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
#[cfg(windows)]
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
    let id = NodeId(node);
    if (node as usize) < tree.len() && !tree.is_removed(id) {
        Ok(id)
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

/// Checks the optional directory header height; `None` means no header.
pub fn validate_header(header: Option<f32>) -> Result<f32, String> {
    match header {
        None => Ok(0.0),
        Some(h) if h.is_finite() && (0.0..=MAX_HEADER).contains(&h) => Ok(h),
        Some(_) => Err("invalid treemap header".to_string()),
    }
}

/// Adds the name, size and parent of each node to its layout rectangle.
pub fn treemap_rects(tree: &Tree, rects: Vec<Rect>) -> Vec<TreemapRect> {
    rects
        .into_iter()
        .map(|r| {
            let id = NodeId(r.id);
            TreemapRect {
                id: r.id,
                parent: tree.parent(id).map(|p| p.0),
                name: tree.name(id).to_string(),
                size: tree.total(id).size,
                x: r.x,
                y: r.y,
                w: r.w,
                h: r.h,
                depth: r.depth,
                is_dir: r.is_dir,
            }
        })
        .collect()
}

/// Clones the current tree out of the state; the lock is released before returning.
fn current_tree(state: &AppState, scan_id: Option<u64>) -> Result<Arc<Tree>, String> {
    let guard = state
        .tree
        .read()
        .map_err(|_| "internal error: state poisoned")?;
    let scan = guard.as_ref().ok_or("no scan yet")?;
    if scan_id.is_some_and(|id| id != scan.id) {
        return Err("scan results have changed".to_string());
    }
    Ok(Arc::clone(&scan.tree))
}

/// Lists the mounted volumes.
#[tauri::command(async)]
pub fn list_drives() -> Vec<VolumeSummary> {
    let volumes = bf_scan::list_volumes();
    #[cfg(target_os = "macos")]
    let volumes = {
        let home = std::env::home_dir()
            .filter(|path| path.is_dir())
            .and_then(|path| path.into_os_string().into_string().ok());
        let home = home.map(|path| VolumeSummary {
            path,
            label: "Home folder".to_string(),
            fs: String::new(),
            total: 0,
            free: 0,
        });
        home.into_iter().chain(volumes).collect()
    };
    volumes
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub async fn pick_folder(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    tauri::async_runtime::spawn_blocking(move || {
        app.dialog()
            .file()
            .blocking_pick_folder()
            .map(|folder| {
                folder
                    .into_path()
                    .map_err(|err| err.to_string())?
                    .into_os_string()
                    .into_string()
                    .map_err(|_| "This folder name cannot be represented as Unicode.".to_owned())
            })
            .transpose()
    })
    .await
    .map_err(|err| err.to_string())?
}

#[cfg(windows)]
#[tauri::command]
pub fn pick_folder() -> Result<Option<String>, String> {
    Err("Folder picker is available on macOS.".to_owned())
}

/// Whether the process runs with administrator rights.
#[tauri::command]
pub fn is_elevated() -> bool {
    bf_scan::is_elevated()
}

/// Restarts the app with administrator rights, optionally opening `target` for scanning.
/// Exits the app on success; `Err("declined")` when the user refuses the UAC prompt.
#[cfg(windows)]
#[tauri::command(async)]
pub fn restart_elevated(app: AppHandle, target: Option<String>) -> Result<(), String> {
    let args = elevation::build_elevated_args(target.as_deref());
    match elevation::relaunch_elevated(&args) {
        Ok(()) => {
            app.exit(0);
            Ok(())
        }
        Err(ElevateError::Declined) => Err("declined".to_string()),
        Err(ElevateError::Other(message)) => Err(message),
    }
}

#[cfg(target_os = "macos")]
#[tauri::command]
pub fn restart_elevated() -> Result<(), String> {
    Err("Elevation is available on Windows.".to_owned())
}

/// The `--scan` startup argument; returned once, then `None`.
#[tauri::command]
pub fn take_startup_scan(state: State<'_, AppState>) -> Option<String> {
    state
        .startup_scan
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

/// Current user settings.
#[tauri::command]
pub fn get_settings() -> Settings {
    store::load_default_path()
}

/// Saves user settings.
#[tauri::command]
pub fn set_settings(settings: Settings) -> Result<(), String> {
    store::save_default_path(&settings).map_err(|e| e.to_string())
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
                    Ok(mut slot) => *slot = Some(CompletedScan { id: scan_id, tree }),
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
    scan_id: Option<u64>,
) -> Result<Vec<NodeView>, String> {
    let tree = current_tree(&state, scan_id)?;
    let id = node_id_checked(&tree, node)?;
    Ok(tree.children(id, sort, offset as usize, limit.min(MAX_CHILDREN) as usize))
}

/// Details of one node.
#[tauri::command]
pub fn get_node(state: State<'_, AppState>, node: u32) -> Result<NodeView, String> {
    let tree = current_tree(&state, None)?;
    let id = node_id_checked(&tree, node)?;
    Ok(tree.view(id))
}

/// Full path of a node.
#[tauri::command]
pub fn get_path(state: State<'_, AppState>, node: u32) -> Result<String, String> {
    let tree = current_tree(&state, None)?;
    let id = node_id_checked(&tree, node)?;
    Ok(display_path(&tree, id))
}

/// Walks toward the root without recursion, even for extremely deep trees.
fn lineage(tree: &Tree, id: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    std::iter::successors(Some(id), |&item| tree.parent(item))
}

fn display_path(tree: &Tree, id: NodeId) -> String {
    let mut parts: Vec<_> = lineage(tree, id).map(|item| tree.name(item)).collect();
    parts.reverse();
    let mut path = parts[0].to_owned();
    for name in &parts[1..] {
        if !path.ends_with(std::path::MAIN_SEPARATOR) {
            path.push(std::path::MAIN_SEPARATOR);
        }
        path.push_str(name);
    }
    path
}

fn native_path(tree: &Tree, id: NodeId) -> Result<PathBuf, String> {
    for item in lineage(tree, id) {
        let name = tree.name(item);
        if tree.flags(item).contains(NodeFlags::SYNTHETIC)
            || name.contains('\u{fffd}')
            || (item != tree.root() && {
                let mut components = Path::new(name).components();
                !matches!(components.next(), Some(Component::Normal(part)) if part == name)
                    || components.next().is_some()
            })
            || (cfg!(windows) && item != tree.root() && name.contains(':'))
        {
            return Err("This entry has no usable filesystem path.".to_owned());
        }
    }

    let path = PathBuf::from(display_path(tree, id));
    #[cfg(windows)]
    let path = if id == tree.root() && matches!(tree.name(id).as_bytes(), [_, b':']) {
        PathBuf::from(format!("{}\\", tree.name(id)))
    } else {
        path
    };

    if path.is_absolute() {
        Ok(path)
    } else {
        Err("This entry has no usable filesystem path.".to_owned())
    }
}

#[derive(Serialize)]
pub struct FileMatch {
    id: u32,
    name: String,
    path: String,
    size: u64,
}

#[derive(Serialize)]
pub struct SearchResults {
    total: usize,
    files: Vec<FileMatch>,
}

#[tauri::command]
pub async fn search_files(
    state: State<'_, AppState>,
    scan_id: u64,
    filter: FileFilter,
) -> Result<SearchResults, String> {
    let tree = current_tree(&state, Some(scan_id))?;
    tauri::async_runtime::spawn_blocking(move || {
        let (total, ids) = tree.largest_files(&filter, 200);
        let files = ids
            .into_iter()
            .map(|id| FileMatch {
                id: id.0,
                name: tree.name(id).to_owned(),
                path: display_path(&tree, id),
                size: tree.total(id).size,
            })
            .collect();
        SearchResults { total, files }
    })
    .await
    .map_err(|err| err.to_string())
}

#[derive(Serialize)]
pub struct LocationStep {
    id: u32,
    name: String,
    position: Option<usize>,
}

#[tauri::command]
pub async fn locate_node(
    state: State<'_, AppState>,
    scan_id: u64,
    node: u32,
) -> Result<Vec<LocationStep>, String> {
    let tree = current_tree(&state, Some(scan_id))?;
    let id = node_id_checked(&tree, node)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut steps: Vec<_> = lineage(&tree, id)
            .map(|item| LocationStep {
                id: item.0,
                name: tree.name(item).to_owned(),
                position: tree.child_position(item, SortKey::SizeDesc),
            })
            .collect();
        steps.reverse();
        steps
    })
    .await
    .map_err(|err| err.to_string())
}

#[derive(Serialize)]
pub struct NodeDetails {
    node: NodeView,
    path: String,
    has_native_path: bool,
}

#[tauri::command]
pub fn node_details(
    state: State<'_, AppState>,
    scan_id: u64,
    node: u32,
) -> Result<NodeDetails, String> {
    let tree = current_tree(&state, Some(scan_id))?;
    let id = node_id_checked(&tree, node)?;
    let path = native_path(&tree, id);
    Ok(NodeDetails {
        node: tree.view(id),
        has_native_path: path.is_ok(),
        path: path.map_or_else(
            |_| display_path(&tree, id),
            |p| p.to_string_lossy().into_owned(),
        ),
    })
}

fn existing_node_path(state: &AppState, scan_id: u64, node: u32) -> Result<PathBuf, String> {
    let tree = current_tree(state, Some(scan_id))?;
    let path = native_path(&tree, node_id_checked(&tree, node)?)?;
    std::fs::symlink_metadata(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => {
            "This entry no longer exists. Rescan to refresh.".to_owned()
        }
        _ => format!("Unable to access this entry: {err}"),
    })?;
    Ok(path)
}

#[tauri::command(async)]
pub fn copy_node_path(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
    node: u32,
) -> Result<(), String> {
    let path = existing_node_path(&state, scan_id, node)?;
    app.clipboard()
        .write_text(path.to_string_lossy())
        .map_err(|err| err.to_string())
}

#[tauri::command(async)]
pub fn reveal_node_in_file_manager(
    state: State<'_, AppState>,
    scan_id: u64,
    node: u32,
) -> Result<(), String> {
    let path = existing_node_path(&state, scan_id, node)?;
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|err| err.to_string())
}

/// What `move_node_to_trash` removed from the tree.
#[derive(Serialize, Debug, PartialEq, Eq)]
pub struct TrashResult {
    bytes: u64,
    files: u64,
    dirs: u64,
    /// Parent of the removed node; the UI selects it afterwards.
    parent: u32,
}

/// The path of a node that may be moved to the trash, derived only from the tree.
///
/// Refuses the scanned root, filesystem roots and entries without a usable native path.
pub fn trash_target(tree: &Tree, node: u32) -> Result<PathBuf, String> {
    let id = node_id_checked(tree, node)?;
    if id == tree.root() {
        return Err("The scanned root cannot be moved to the trash.".to_owned());
    }
    let path = native_path(tree, id)?;
    if path.parent().is_none() || path.file_name().is_none() {
        return Err("A drive or filesystem root cannot be moved to the trash.".to_owned());
    }
    Ok(path)
}

/// Moves a node to the Recycle Bin / Trash and removes it from the in-memory tree.
#[tauri::command]
pub async fn move_node_to_trash(
    state: State<'_, AppState>,
    scan_id: u64,
    node: u32,
) -> Result<TrashResult, String> {
    let path = {
        let slot = state
            .scan
            .lock()
            .map_err(|_| "internal error: state poisoned")?;
        if slot.id != scan_id {
            return Err("A scan is in progress or the results have changed.".to_owned());
        }
        let tree = current_tree(&state, Some(scan_id))?;
        trash_target(&tree, node)?
    };

    let target = path.clone();
    tauri::async_runtime::spawn_blocking(move || trash::delete(&target))
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| format!("Could not move the item to the trash: {err}"))?;

    match std::fs::symlink_metadata(&path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(format!("Unable to verify the item was removed: {err}")),
        Ok(_) => return Err("The item is still on disk after moving it to the trash.".to_owned()),
    }

    let slot = state
        .scan
        .lock()
        .map_err(|_| "internal error: state poisoned")?;
    if slot.id != scan_id {
        return Err("The item was moved to the trash, but a new scan has started.".to_owned());
    }
    let mut guard = state
        .tree
        .write()
        .map_err(|_| "internal error: state poisoned")?;
    let scan = guard.as_mut().ok_or("no scan yet")?;
    if scan.id != scan_id {
        return Err("The item was moved to the trash, but the scan results changed.".to_owned());
    }
    let id = node_id_checked(&scan.tree, node)?;
    let parent = scan.tree.parent(id).map_or(0, |p| p.0);
    // Copy-on-write: clones the tree only if a reader still holds the previous `Arc`.
    let removed = Arc::make_mut(&mut scan.tree)
        .remove_subtree(id)
        .map_err(|err| err.to_string())?;
    Ok(TrashResult {
        bytes: removed.bytes,
        files: removed.files,
        dirs: removed.dirs,
        parent,
    })
}

/// Treemap rectangles (at most 20 000) of the subtree under `node`, in a `width` x `height` canvas.
/// `header` (optional, pixels) reserves a name strip at the top of large non-root directories.
#[tauri::command(async)]
pub fn get_treemap(
    state: State<'_, AppState>,
    node: u32,
    width: f32,
    height: f32,
    header: Option<f32>,
) -> Result<Vec<TreemapRect>, String> {
    validate_canvas(width, height)?;
    let dir_header = validate_header(header)?;
    let tree = current_tree(&state, None)?;
    let id = node_id_checked(&tree, node)?;
    let opts = TreemapOptions {
        width,
        height,
        dir_header,
        ..Default::default()
    };
    Ok(treemap_rects(&tree, tree.treemap(id, &opts)))
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
    fn trash_target_refuses_root_and_removed_nodes() {
        let mut tree = tiny_tree();
        assert!(trash_target(&tree, 0).is_err());
        assert!(trash_target(&tree, 99).is_err());
        tree.remove_subtree(NodeId(1)).unwrap();
        assert!(trash_target(&tree, 1).is_err());
    }

    #[test]
    #[cfg(windows)]
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
    fn header_is_optional_finite_and_bounded() {
        assert_eq!(validate_header(None), Ok(0.0));
        assert_eq!(validate_header(Some(16.0)), Ok(16.0));
        for bad in [-1.0, f32::NAN, f32::INFINITY, MAX_HEADER + 1.0] {
            assert!(validate_header(Some(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn treemap_rects_carry_name_size_and_parent() {
        let tree = tiny_tree();
        let opts = TreemapOptions {
            width: 100.0,
            height: 100.0,
            ..Default::default()
        };
        let rects = treemap_rects(&tree, tree.treemap(tree.root(), &opts));
        assert_eq!(rects.len(), 2);
        assert_eq!((rects[0].parent, rects[0].size), (None, 10));
        assert_eq!(rects[1].parent, Some(0));
        assert_eq!(rects[1].name, "a.txt");
        assert_eq!(rects[1].size, 10);
        assert!(!rects[1].is_dir);
    }

    #[test]
    fn native_actions_are_bound_to_the_scan_and_existing_paths() {
        let folder = tempfile::tempdir().unwrap();
        let file = folder.path().join("a file.txt");
        std::fs::write(&file, b"x").unwrap();
        let mut builder = TreeBuilder::new(0, folder.path().to_str().unwrap());
        for (key, parent_key, name) in [(1, 0, "a file.txt"), (2, 999, "orphan.txt")] {
            builder.push(EntryRecord {
                key,
                parent_key,
                name: name.to_string(),
                flags: NodeFlags::empty(),
                size: 1,
                alloc: 1,
                mtime: 0,
            });
        }
        let tree = Arc::new(builder.finish());
        let id = tree
            .children_ids(tree.root())
            .find(|&id| tree.name(id) == "a file.txt")
            .unwrap();
        let synthetic = tree
            .children_ids(tree.root())
            .find(|&id| tree.flags(id).contains(NodeFlags::SYNTHETIC))
            .unwrap();
        let state = AppState::default();
        *state.tree.write().unwrap() = Some(CompletedScan { id: 7, tree });
        assert_eq!(existing_node_path(&state, 7, id.0), Ok(file.clone()));
        assert!(existing_node_path(&state, 6, id.0).is_err());
        assert!(existing_node_path(&state, 7, synthetic.0).is_err());
        std::fs::remove_file(&file).unwrap();
        assert!(
            existing_node_path(&state, 7, id.0)
                .unwrap_err()
                .contains("no longer exists")
        );
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
