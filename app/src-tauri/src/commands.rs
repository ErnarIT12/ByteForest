//! IPC commands called by the front end, plus the pure helpers they rely on.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use bf_core::{
    ExclusionSet, FileFilter, NodeFlags, NodeId, Rect, ReportOptions, SnapshotDiff, SnapshotLimits,
    SortKey, Tree, TreemapOptions, diff, read_snapshot_with_limits, write_csv, write_snapshot,
};
use bf_scan::{CancelToken, ScanError, ScanRoot, ScanStats, VolumeSummary};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::dto::{
    BaselineInfo, ChangeDto, ExcludedInfo, NodeViewDto, ScanDone, ScanErrorEvent,
    ScanProgressEvent, TreemapRect,
};
#[cfg(windows)]
use crate::elevation::{self, ElevateError};
use crate::settings::{self as store, Settings};
use crate::state::{AppState, Baseline, CompletedScan};
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
                category: r.category,
            }
        })
        .collect()
}

/// Clones the current tree and its scan id out of the state; the lock is released before
/// returning.
fn current_scan(state: &AppState, scan_id: Option<u64>) -> Result<(u64, Arc<Tree>), String> {
    let guard = state
        .tree
        .read()
        .map_err(|_| "internal error: state poisoned")?;
    let scan = guard.as_ref().ok_or("no scan yet")?;
    if scan_id.is_some_and(|id| id != scan.id) {
        return Err("scan results have changed".to_string());
    }
    Ok((scan.id, Arc::clone(&scan.tree)))
}

/// Clones the current tree out of the state; the lock is released before returning.
fn current_tree(state: &AppState, scan_id: Option<u64>) -> Result<Arc<Tree>, String> {
    current_scan(state, scan_id).map(|(_, tree)| tree)
}

/// The comparison for scan `scan_id`, if one is active. Takes only the `baseline` lock.
fn baseline_diff(state: &AppState, scan_id: u64) -> Option<Arc<SnapshotDiff>> {
    let guard = state
        .baseline
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard
        .as_ref()
        .filter(|b| b.scan_id == scan_id)
        .map(|b| Arc::clone(&b.diff))
}

/// Drops any comparison. Callers hold at most the `scan` and `tree` locks.
fn drop_baseline(state: &AppState) {
    *state
        .baseline
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// `C:` for a volume, the absolute path for a folder.
fn scan_label(root: &ScanRoot) -> String {
    match root {
        #[cfg(windows)]
        ScanRoot::Volume(letter) => format!("{letter}:"),
        ScanRoot::Folder(path) => path.to_string_lossy().into_owned(),
    }
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

/// A well-known folder of the current user, offered as a quick scan location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UserFolder {
    /// `desktop`, `downloads` or `documents`.
    pub kind: &'static str,
    pub name: &'static str,
    pub path: String,
}

/// Keeps the candidates that are existing folders with a Unicode path, once each, in the
/// fixed order Desktop, Downloads, Documents (as listed in `candidates`).
fn existing_user_folders(
    candidates: Vec<(&'static str, &'static str, Option<PathBuf>)>,
) -> Vec<UserFolder> {
    let mut folders: Vec<UserFolder> = Vec::new();
    for (kind, name, path) in candidates {
        let Some(path) = path.filter(|path| path.is_dir()) else {
            continue;
        };
        let Ok(path) = path.into_os_string().into_string() else {
            continue;
        };
        if folders.iter().any(|known| known.path == path) {
            continue;
        }
        folders.push(UserFolder { kind, name, path });
    }
    folders
}

/// Desktop, Downloads and Documents of the current user. The system resolves redirected known
/// folders (for example a Documents folder moved to OneDrive); missing ones are left out.
#[tauri::command]
pub fn user_folders(app: AppHandle) -> Vec<UserFolder> {
    let paths = app.path();
    existing_user_folders(vec![
        ("desktop", "Desktop", paths.desktop_dir().ok()),
        ("downloads", "Downloads", paths.download_dir().ok()),
        ("documents", "Documents", paths.document_dir().ok()),
    ])
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
    store::save_default_path(&settings.sanitized()).map_err(|e| e.to_string())
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
        // The comparison belongs to the previous scan's tree.
        drop_baseline(&state);
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
    let label = scan_label(&root);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let result = bf_scan::scan_auto(&root, &progress, &token);
        finish_scan(&app, scan_id, &token, label, result)
    }));
    if outcome.is_err() {
        tracing::error!("scan {scan_id} panicked");
        report(&app, scan_id, |_| {
            Some(("internal error: scan crashed".to_string(), None))
        });
    }
}

/// Cuts the user's exclusions out of a fresh tree and out of its stats. `label` is the scan
/// root (`C:` or a folder path). `None` when nothing was removed.
fn exclude_from_scan(
    tree: &mut Tree,
    stats: &mut ScanStats,
    label: &str,
    patterns: &[String],
) -> Option<ExcludedInfo> {
    if patterns.is_empty() {
        return None;
    }
    let set = ExclusionSet::from_patterns(patterns, label, case_insensitive_names());
    let cut = tree.apply_exclusions(&set, case_insensitive_names());
    stats.files = stats.files.saturating_sub(cut.files);
    stats.dirs = stats.dirs.saturating_sub(cut.dirs);
    stats.bytes = stats.bytes.saturating_sub(cut.bytes);
    (cut.items > 0).then_some(ExcludedInfo {
        items: cut.items,
        bytes: cut.bytes,
    })
}

/// Stores the tree and emits the outcome, but only while `scan_id` is still the current scan.
fn finish_scan(
    app: &AppHandle,
    scan_id: u64,
    token: &CancelToken,
    label: String,
    result: Result<(Tree, bf_scan::ScanStats), ScanError>,
) {
    match result {
        Ok((mut tree, mut stats)) if !token.is_cancelled() => {
            let patterns = store::load_default_path().exclusions;
            let excluded = exclude_from_scan(&mut tree, &mut stats, &label, &patterns);
            let tree = Arc::new(tree);
            let root = tree.view(tree.root());
            tracing::info!(
                "scan {scan_id} done: {} files, {} dirs",
                stats.files,
                stats.dirs
            );
            report(app, scan_id, |state| {
                match state.tree.write() {
                    Ok(mut slot) => {
                        *slot = Some(CompletedScan {
                            id: scan_id,
                            tree,
                            target: label,
                        });
                    }
                    Err(_) => return Some(("internal error: state poisoned".to_string(), None)),
                }
                drop_baseline(state);
                Some((
                    String::new(),
                    Some(ScanDone {
                        scan_id,
                        stats,
                        root,
                        excluded,
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
) -> Result<Vec<NodeViewDto>, String> {
    let (current_id, tree) = current_scan(&state, scan_id)?;
    let id = node_id_checked(&tree, node)?;
    let diff = baseline_diff(&state, current_id);
    Ok(tree
        .children(id, sort, offset as usize, limit.min(MAX_CHILDREN) as usize)
        .into_iter()
        .map(|view| NodeViewDto::new(view, diff.as_deref(), &tree))
        .collect())
}

/// Details of one node.
#[tauri::command]
pub fn get_node(state: State<'_, AppState>, node: u32) -> Result<NodeViewDto, String> {
    let (scan_id, tree) = current_scan(&state, None)?;
    let id = node_id_checked(&tree, node)?;
    let diff = baseline_diff(&state, scan_id);
    Ok(NodeViewDto::new(tree.view(id), diff.as_deref(), &tree))
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
    is_dir: bool,
}

#[derive(Serialize)]
pub struct SearchResults {
    total: usize,
    total_bytes: u64,
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
        let found = tree.largest_files(&filter, 200);
        let files = found
            .ids
            .into_iter()
            .map(|id| FileMatch {
                id: id.0,
                name: tree.name(id).to_owned(),
                path: display_path(&tree, id),
                size: tree.total(id).size,
                is_dir: tree.flags(id).contains(NodeFlags::DIR),
            })
            .collect();
        SearchResults {
            total: found.total,
            total_bytes: found.total_bytes,
            files,
        }
    })
    .await
    .map_err(|err| err.to_string())
}

/// Per-category and per-extension totals of a folder (`top` is clamped to 1..=50, default 10).
#[tauri::command]
pub async fn folder_breakdown(
    state: State<'_, AppState>,
    scan_id: u64,
    node: u32,
    top: Option<u32>,
) -> Result<bf_core::Breakdown, String> {
    let tree = current_tree(&state, Some(scan_id))?;
    let id = node_id_checked(&tree, node)?;
    let top = top.unwrap_or(10).clamp(1, 50) as usize;
    tauri::async_runtime::spawn_blocking(move || tree.extension_breakdown(id, top))
        .await
        .map_err(|err| err.to_string())?
        .ok_or_else(|| "unknown node".to_string())
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
    node: NodeViewDto,
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
    let diff = baseline_diff(&state, scan_id);
    let path = native_path(&tree, id);
    Ok(NodeDetails {
        node: NodeViewDto::new(tree.view(id), diff.as_deref(), &tree),
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

    let (result, recompute) = {
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
            return Err(
                "The item was moved to the trash, but the scan results changed.".to_owned(),
            );
        }
        let id = node_id_checked(&scan.tree, node)?;
        let parent = scan.tree.parent(id).map_or(0, |p| p.0);
        // Copy-on-write: clones the tree only if a reader still holds the previous `Arc`.
        let removed = Arc::make_mut(&mut scan.tree)
            .remove_subtree(id)
            .map_err(|err| err.to_string())?;
        // The comparison must reflect the removal; it is recomputed below, off the locks.
        let recompute = state
            .baseline
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .filter(|b| b.scan_id == scan_id)
            .map(|b| (Arc::clone(&b.base), Arc::clone(&scan.tree)));
        (
            TrashResult {
                bytes: removed.bytes,
                files: removed.files,
                dirs: removed.dirs,
                parent,
            },
            recompute,
        )
    };
    if let Some((base, current)) = recompute {
        refresh_baseline(&state, scan_id, base, current).await;
    }
    Ok(result)
}

/// Recomputes the comparison for `current` and swaps it in, unless the scan or the baseline
/// changed in the meantime. A failure leaves the old diff, whose per-node sizes still follow
/// the live tree; only the summary and the "removed" counts lag until the next compare.
async fn refresh_baseline(state: &AppState, scan_id: u64, base: Arc<Tree>, current: Arc<Tree>) {
    let base_for_diff = Arc::clone(&base);
    let fresh = tauri::async_runtime::spawn_blocking(move || {
        diff(&base_for_diff, &current, case_insensitive_names())
    })
    .await;
    let Ok(fresh) = fresh else {
        tracing::warn!("recomputing the snapshot comparison failed");
        return;
    };
    let slot = state
        .scan
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if slot.id != scan_id {
        return;
    }
    let mut guard = state
        .baseline
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(b) = guard
        .as_mut()
        .filter(|b| b.scan_id == scan_id && Arc::ptr_eq(&b.base, &base))
    {
        b.diff = Arc::new(fresh);
    }
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

/// Largest snapshot file `compare_with_snapshot` opens.
const MAX_SNAPSHOT_FILE: u64 = 4 * 1024 * 1024 * 1024;

/// Most rows `top_changes` returns.
const MAX_CHANGES: u32 = 200;

/// Whether file names compare case-insensitively on the supported platforms' usual file
/// systems (NTFS and APFS defaults).
fn case_insensitive_names() -> bool {
    cfg!(windows) || cfg!(target_os = "macos")
}

fn unix_ms_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// `YYYYMMDD-HHMM` in UTC.
fn utc_stamp(unix_ms: i64) -> String {
    let secs = unix_ms.div_euclid(1000);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Civil-from-days (proleptic Gregorian), after Howard Hinnant.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}",
        rem / 3600,
        rem % 3600 / 60
    )
}

/// `byteforest-<target>-<YYYYMMDD-HHMM>.bfsnap` with the target reduced to file-name-safe
/// characters.
pub fn default_snapshot_name(target: &str, unix_ms: i64) -> String {
    format!(
        "byteforest-{}-{}.bfsnap",
        file_name_part(target),
        utc_stamp(unix_ms)
    )
}

/// `byteforest-<name>-<YYYYMMDD-HHMM>.csv` with the name reduced to file-name-safe
/// characters.
pub fn default_report_name(name: &str, unix_ms: i64) -> String {
    format!(
        "byteforest-{}-{}.csv",
        file_name_part(name),
        utc_stamp(unix_ms)
    )
}

/// Letters and digits of `text` with every other run replaced by one `-`, at most 40
/// characters; `scan` when nothing is left.
fn file_name_part(text: &str) -> String {
    let mut safe = String::new();
    for ch in text.chars() {
        let ch = if ch.is_alphanumeric() { ch } else { '-' };
        if ch != '-' || !safe.ends_with('-') {
            safe.push(ch);
        }
    }
    let safe: String = safe.trim_matches('-').chars().take(40).collect();
    let safe = safe.trim_end_matches('-');
    if safe.is_empty() {
        "scan".to_string()
    } else {
        safe.to_string()
    }
}

/// Writes the snapshot to `<path>.tmp` and renames it over `path`.
fn write_snapshot_file(
    tree: &Tree,
    label: &str,
    created_unix_ms: i64,
    path: &Path,
) -> Result<(), String> {
    use std::io::Write;
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    let write = || -> Result<(), String> {
        let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut w = std::io::BufWriter::new(file);
        write_snapshot(tree, label, created_unix_ms, &mut w).map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        drop(w);
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not save the snapshot: {e}")
    })
}

/// What was loaded from a snapshot file and compared with the current tree.
struct LoadedBaseline {
    label: String,
    created_unix_ms: i64,
    base: Arc<Tree>,
    diff: SnapshotDiff,
}

/// Reads a snapshot file (refusing files over [`MAX_SNAPSHOT_FILE`]) and compares it with
/// `current`.
fn load_baseline(path: &Path, current: &Tree) -> Result<LoadedBaseline, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("Cannot open the snapshot: {e}"))?;
    if meta.len() > MAX_SNAPSHOT_FILE {
        return Err("This snapshot file is too large to open.".to_owned());
    }
    let file = std::fs::File::open(path).map_err(|e| format!("Cannot open the snapshot: {e}"))?;
    let snap = read_snapshot_with_limits(
        std::io::BufReader::new(file),
        SnapshotLimits::for_compressed_len(meta.len()),
    )
    .map_err(|e| format!("Cannot read the snapshot: {e}"))?;
    let diff = diff(&snap.tree, current, case_insensitive_names());
    Ok(LoadedBaseline {
        label: snap.label,
        created_unix_ms: snap.created_unix_ms,
        base: Arc::new(snap.tree),
        diff,
    })
}

/// Saves the scan as a snapshot file chosen in a save dialog. Returns the path written, or
/// `None` when the dialog was cancelled.
#[tauri::command]
pub async fn save_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tree, target) = {
        let guard = state
            .tree
            .read()
            .map_err(|_| "internal error: state poisoned")?;
        let scan = guard
            .as_ref()
            .filter(|scan| scan.id == scan_id)
            .ok_or("scan results have changed")?;
        (Arc::clone(&scan.tree), scan.target.clone())
    };
    let created = unix_ms_now();
    let name = default_snapshot_name(&target, created);
    let dir = app
        .path()
        .app_local_data_dir()
        .ok()
        .map(|dir| dir.join("snapshots"));
    if let Some(dir) = &dir {
        let _ = std::fs::create_dir_all(dir);
    }

    let dialog_app = app.clone();
    let chosen = tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = dialog_app
            .dialog()
            .file()
            .add_filter("ByteForest snapshot", &["bfsnap"])
            .set_file_name(name);
        if let Some(dir) = dir.filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        dialog.blocking_save_file()
    })
    .await
    .map_err(|err| err.to_string())?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let mut path = chosen.into_path().map_err(|err| err.to_string())?;
    if path.extension().is_none() {
        path.set_extension("bfsnap");
    }

    tauri::async_runtime::spawn_blocking(move || {
        write_snapshot_file(&tree, &target, created, &path).map(|()| path)
    })
    .await
    .map_err(|err| err.to_string())?
    .map(|path| Some(path.to_string_lossy().into_owned()))
}

/// Folder levels an exported report covers.
const REPORT_DEPTH: u8 = 3;

/// Most rows an exported report holds.
const REPORT_ROWS: usize = 100_000;

/// Writes the report of `id` to `<path>.tmp` and renames it over `path`. A `.json` extension
/// gives JSON, anything else CSV.
fn write_report_file(tree: &Tree, id: NodeId, path: &Path) -> Result<(), String> {
    use std::io::Write;
    let opts = ReportOptions {
        max_depth: REPORT_DEPTH,
        max_rows: REPORT_ROWS,
        path_separator: std::path::MAIN_SEPARATOR,
    };
    let report = tree
        .report(id, opts)
        .ok_or_else(|| "Could not export the report: unknown node".to_string())?;
    let json = path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
    let mut tmp_name = path.as_os_str().to_owned();
    tmp_name.push(".tmp");
    let tmp = PathBuf::from(tmp_name);
    let write = || -> Result<(), String> {
        let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut w = std::io::BufWriter::new(file);
        if json {
            serde_json::to_writer(&mut w, &report).map_err(|e| e.to_string())?;
        } else {
            write_csv(&report, ',', &mut w).map_err(|e| e.to_string())?;
        }
        w.flush().map_err(|e| e.to_string())?;
        drop(w);
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())
    };
    write().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("Could not export the report: {e}")
    })
}

/// Exports a CSV or JSON report of a folder (and its biggest contents) to a file chosen in a
/// save dialog. Returns the path written, or `None` when the dialog was cancelled.
#[tauri::command]
pub async fn export_report(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
    node: u32,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let tree = current_tree(&state, Some(scan_id))?;
    let id = node_id_checked(&tree, node)?;
    let name = default_report_name(tree.name(id), unix_ms_now());
    let dir = app
        .path()
        .document_dir()
        .or_else(|_| app.path().download_dir())
        .ok();

    let dialog_app = app.clone();
    let chosen = tauri::async_runtime::spawn_blocking(move || {
        let mut dialog = dialog_app
            .dialog()
            .file()
            .add_filter("CSV (spreadsheet)", &["csv"])
            .add_filter("JSON", &["json"])
            .set_file_name(name);
        if let Some(dir) = dir.filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        dialog.blocking_save_file()
    })
    .await
    .map_err(|err| err.to_string())?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let mut path = chosen.into_path().map_err(|err| err.to_string())?;
    if path.extension().is_none() {
        path.set_extension("csv");
    }

    tauri::async_runtime::spawn_blocking(move || write_report_file(&tree, id, &path).map(|()| path))
        .await
        .map_err(|err| err.to_string())?
        .map(|path| Some(path.to_string_lossy().into_owned()))
}

/// Opens a snapshot chosen in an open dialog and compares the scan with it. Returns `None`
/// when the dialog was cancelled.
#[tauri::command]
pub async fn compare_with_snapshot(
    app: AppHandle,
    state: State<'_, AppState>,
    scan_id: u64,
) -> Result<Option<BaselineInfo>, String> {
    use tauri_plugin_dialog::DialogExt;
    let tree = current_tree(&state, Some(scan_id))?;

    let dialog_app = app.clone();
    let chosen = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("ByteForest snapshot", &["bfsnap"])
            .blocking_pick_file()
    })
    .await
    .map_err(|err| err.to_string())?;
    let Some(chosen) = chosen else {
        return Ok(None);
    };
    let path = chosen.into_path().map_err(|err| err.to_string())?;

    let compared = Arc::clone(&tree);
    let loaded = tauri::async_runtime::spawn_blocking(move || load_baseline(&path, &compared))
        .await
        .map_err(|err| err.to_string())??;

    let summary = loaded.diff.summary();
    let info = BaselineInfo {
        label: loaded.label.clone(),
        created_unix_ms: loaded.created_unix_ms,
        total_delta: summary.total_delta,
        added: summary.added.into(),
        removed: summary.removed.into(),
    };
    // Lock order: scan, tree, baseline. The diff is only valid for the exact tree it was
    // computed from.
    let slot = state
        .scan
        .lock()
        .map_err(|_| "internal error: state poisoned")?;
    let current = state
        .tree
        .read()
        .map_err(|_| "internal error: state poisoned")?;
    let unchanged = slot.id == scan_id
        && current
            .as_ref()
            .is_some_and(|scan| scan.id == scan_id && Arc::ptr_eq(&scan.tree, &tree));
    if !unchanged {
        return Err("The scan results changed while the snapshot was loading.".to_owned());
    }
    *state
        .baseline
        .write()
        .map_err(|_| "internal error: state poisoned")? = Some(Baseline {
        scan_id,
        base: loaded.base,
        diff: Arc::new(loaded.diff),
    });
    Ok(Some(info))
}

/// Stops comparing with a snapshot.
#[tauri::command]
pub fn clear_baseline(state: State<'_, AppState>) {
    drop_baseline(&state);
}

/// The nodes that grew or are new since the snapshot, largest increase first (`limit` is
/// clamped to 1..=200).
#[tauri::command]
pub async fn top_changes(
    state: State<'_, AppState>,
    scan_id: u64,
    limit: u32,
) -> Result<Vec<ChangeDto>, String> {
    let tree = current_tree(&state, Some(scan_id))?;
    let diff = baseline_diff(&state, scan_id).ok_or("Not comparing with a snapshot.")?;
    let limit = limit.clamp(1, MAX_CHANGES) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        diff.top_changes(&tree, limit)
            .into_iter()
            .map(|(id, entry)| ChangeDto {
                id: id.0,
                name: tree.name(id).to_owned(),
                path: display_path(&tree, id),
                status: entry.status.into(),
                base: entry.base,
                current: entry.current,
                delta: entry.delta,
            })
            .collect()
    })
    .await
    .map_err(|err| err.to_string())
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
    fn user_folders_keep_order_and_skip_missing_or_duplicate_paths() {
        let dir = tempfile::tempdir().unwrap();
        let desktop = dir.path().join("Desktop");
        let documents = dir.path().join("Documents");
        let file = dir.path().join("Downloads");
        std::fs::create_dir(&desktop).unwrap();
        std::fs::create_dir(&documents).unwrap();
        std::fs::write(&file, b"not a folder").unwrap();

        let found = existing_user_folders(vec![
            ("desktop", "Desktop", Some(desktop.clone())),
            ("downloads", "Downloads", Some(file)),
            ("documents", "Documents", Some(documents.clone())),
            ("extra", "Extra", Some(documents)),
            ("missing", "Missing", Some(dir.path().join("nope"))),
            ("none", "None", None),
        ]);
        let kinds: Vec<_> = found.iter().map(|folder| folder.kind).collect();
        assert_eq!(kinds, ["desktop", "documents"]);
        assert_eq!(found[0].path, desktop.to_string_lossy());
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
        assert_eq!(rects[0].category, None);
        assert_eq!(rects[1].category, Some(bf_core::FileCategory::Other));
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
        *state.tree.write().unwrap() = Some(CompletedScan {
            id: 7,
            tree,
            target: String::new(),
        });
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

    #[test]
    fn snapshot_names_are_safe_and_stamped() {
        assert_eq!(
            default_snapshot_name("C:", 1_700_000_000_000),
            "byteforest-C-20231114-2213.bfsnap"
        );
        assert_eq!(
            default_snapshot_name(r"C:\Users\a b/..\x", 0),
            "byteforest-C-Users-a-b-x-19700101-0000.bfsnap"
        );
        assert_eq!(
            default_snapshot_name("///", 0),
            "byteforest-scan-19700101-0000.bfsnap"
        );
        let long = default_snapshot_name(&"x".repeat(500), 0);
        assert!(long.len() < 80, "{long}");
    }

    #[test]
    fn report_names_are_safe_and_stamped() {
        assert_eq!(
            default_report_name("my docs", 1_700_000_000_000),
            "byteforest-my-docs-20231114-2213.csv"
        );
        assert_eq!(
            default_report_name("..", 0),
            "byteforest-scan-19700101-0000.csv"
        );
    }

    #[test]
    fn exclusions_are_cut_from_tree_and_stats() {
        let mut tree = tiny_tree();
        let mut stats = ScanStats {
            files: 1,
            dirs: 0,
            bytes: 10,
            elapsed_ms: 0,
            method: bf_scan::ScanMethod::Walk,
            fallback_reason: None,
            skipped_records: 0,
            errors: 0,
        };
        let none = exclude_from_scan(&mut tree, &mut stats, "C:", &[]);
        assert!(none.is_none());
        let cut = exclude_from_scan(&mut tree, &mut stats, "C:", &["*.TXT".to_string()]);
        assert_eq!(
            cut,
            Some(ExcludedInfo {
                items: 1,
                bytes: 10
            })
        );
        assert_eq!((stats.files, stats.bytes), (0, 0));
        assert_eq!(tree.total(tree.root()).size, 0);
    }

    #[test]
    fn report_file_is_written_as_csv_or_json_without_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let tree = tiny_tree();
        let csv = dir.path().join("r.csv");
        write_report_file(&tree, tree.root(), &csv).unwrap();
        let text = std::fs::read_to_string(&csv).unwrap();
        assert!(text.starts_with('\u{FEFF}'));
        assert!(text.contains("a.txt"));
        let json = dir.path().join("r.JSON");
        write_report_file(&tree, tree.root(), &json).unwrap();
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        assert_eq!(v["rows"][1]["name"], "a.txt");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }

    #[test]
    fn snapshot_file_round_trips_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.bfsnap");
        let before = tiny_tree();
        write_snapshot_file(&before, "C:", 5, &path).unwrap();
        assert!(!dir.path().join("t.bfsnap.tmp").exists());

        let mut b = TreeBuilder::new(5, "C:");
        for (key, name, size) in [(6, "a.txt", 10), (7, "b.txt", 90)] {
            b.push(EntryRecord {
                key,
                parent_key: 5,
                name: name.into(),
                flags: NodeFlags::empty(),
                size,
                alloc: size,
                mtime: 0,
            });
        }
        let now = b.finish();
        let loaded = load_baseline(&path, &now).unwrap();
        assert_eq!((loaded.label.as_str(), loaded.created_unix_ms), ("C:", 5));
        let summary = loaded.diff.summary();
        assert_eq!(summary.total_delta, 90);
        assert_eq!(summary.added.count, 1);
        let added = now
            .children_ids(now.root())
            .find(|&id| now.name(id) == "b.txt")
            .unwrap();
        let dto = crate::dto::diff_dto(&loaded.diff, &now, added).unwrap();
        assert_eq!(dto.status, crate::dto::DiffKind::Added);
        assert_eq!(dto.delta, 90);
        let unchanged = now
            .children_ids(now.root())
            .find(|&id| now.name(id) == "a.txt")
            .unwrap();
        assert!(crate::dto::diff_dto(&loaded.diff, &now, unchanged).is_none());
    }

    #[test]
    fn corrupt_snapshot_file_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.bfsnap");
        std::fs::write(&path, b"BFSNAP garbage").unwrap();
        let err = load_baseline(&path, &tiny_tree()).err().unwrap();
        assert!(err.starts_with("Cannot read the snapshot"), "{err}");
        assert!(load_baseline(&dir.path().join("missing"), &tiny_tree()).is_err());
    }

    #[test]
    fn baseline_is_only_visible_for_its_own_scan() {
        let state = AppState::default();
        let base = Arc::new(tiny_tree());
        let diff = Arc::new(bf_core::diff(&base, &base, false));
        *state.baseline.write().unwrap() = Some(Baseline {
            scan_id: 3,
            base,
            diff,
        });
        assert!(baseline_diff(&state, 3).is_some());
        assert!(baseline_diff(&state, 4).is_none());
        drop_baseline(&state);
        assert!(baseline_diff(&state, 3).is_none());
    }
}
