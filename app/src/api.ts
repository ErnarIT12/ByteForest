/**
 * Typed wrappers over the Tauri IPC surface of ByteForest.
 *
 * Argument objects use camelCase keys (Tauri converts them to the Rust snake_case names):
 * `node`, `sort`, `offset`, `limit`, `width`, `height`, `target`.
 * Scan events (`scan://progress`, `scan://done`, `scan://error`) all carry `scan_id`, the value
 * `startScan` resolved to. Starting a scan cancels the previous one, so the UI MUST ignore events
 * whose `scan_id` is not the latest id it started (a superseded scan may still report "cancelled").
 * Every command rejects with a plain string message on failure
 * (for example "no scan yet" or "unknown node").
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** Order of a directory's children (serde names of the Rust enum). */
export type SortKey = 'SizeDesc' | 'NameAsc' | 'ModifiedDesc';

export type ScanMethod = 'Mft' | 'Walk';
export type Phase = 'Reading' | 'Parsing' | 'Walking' | 'Building';

/** A well-known folder of the current user (Desktop, Downloads, Documents). */
export interface UserFolder {
  kind: string;
  name: string;
  path: string;
}

export interface VolumeSummary {
  /** Upper-case drive letter, e.g. "C". */
  letter?: string;
  /** Absolute mount or folder path on macOS. */
  path?: string;
  label: string;
  fs: string;
  /** Bytes. */
  total: number;
  /** Bytes. */
  free: number;
}

/** How a node differs from the snapshot being compared against (serde names of the Rust enum). */
export type DiffStatus = 'added' | 'grown' | 'shrunk' | 'unchanged';

/** Count and size of entries that were added or have disappeared. */
export interface GoneInfo {
  bytes: number;
  count: number;
}

/** Comparison result of one node; only present while comparing with a snapshot. */
export interface NodeDiff {
  status: DiffStatus;
  /** Size in the snapshot; null for new nodes. */
  base: number | null;
  /** Bytes gained (positive) or lost (negative) since the snapshot. */
  delta: number;
  /** Things inside a folder that are in the snapshot but gone now. */
  removed: GoneInfo | null;
}

export interface NodeView {
  id: number;
  name: string;
  size: number;
  alloc: number;
  files: number;
  dirs: number;
  is_dir: boolean;
  has_children: boolean;
  /** Windows FILETIME (100-ns ticks since 1601-01-01). */
  mtime: number;
  percent_of_parent: number;
  /** Set by the backend only while comparing with a snapshot, and only when something changed. */
  diff?: NodeDiff | null;
}

/** File category (serde names of the Rust enum). */
export type FileCategory =
  | 'video'
  | 'archive'
  | 'diskimage'
  | 'codebuild'
  | 'media'
  | 'system'
  | 'other';

/** One treemap rectangle, in the pixel space passed to `getTreemap`. */
export interface Rect {
  id: number;
  /** Node id of the parent (null for the tree root). */
  parent: number | null;
  name: string;
  /** Logical size of the subtree in bytes. */
  size: number;
  x: number;
  y: number;
  w: number;
  h: number;
  /** 0 for the requested node itself. */
  depth: number;
  is_dir: boolean;
  /** Category of a file; null for folders. */
  category: FileCategory | null;
  /** Set only while comparing with a snapshot, and only when something changed. */
  diff?: NodeDiff | null;
}

/** Id of the merged "other" arc of a sunburst (`u32::MAX`). */
export const OTHER_ARC_ID = 4294967295;

/** One sunburst ring segment; angles are radians clockwise from 12 o'clock. */
export interface SunburstArc {
  /** Node id, or `OTHER_ARC_ID` for the merged arc of small items (empty `name`). */
  id: number;
  /** Node id of the parent arc (the requested node for the first ring). */
  parent: number;
  name: string;
  size: number;
  /** 1 is the ring around the centre disc. */
  depth: number;
  start: number;
  end: number;
  is_dir: boolean;
  category: FileCategory | null;
  diff?: NodeDiff | null;
}

export interface ScanProgress {
  /** Id returned by `startScan`. */
  scan_id: number;
  files: number;
  dirs: number;
  bytes: number;
  phase: Phase;
}

export interface ScanStats {
  files: number;
  dirs: number;
  bytes: number;
  elapsed_ms: number;
  method: ScanMethod;
  fallback_reason: string | null;
  skipped_records: number;
  errors: number;
}

/** What the scan exclusions cut out of a scan. */
export interface ExcludedInfo {
  items: number;
  bytes: number;
}

export interface ScanDone {
  scan_id: number;
  stats: ScanStats;
  root: NodeView;
  /** Present only when the exclusions removed something. */
  excluded?: ExcludedInfo | null;
}

export interface ScanErrorEvent {
  scan_id: number;
  message: string;
}

/** `scan://error` message of a scan stopped by `cancelScan` or superseded by a newer scan. */
export const SCAN_CANCELLED = 'cancelled';

/** Maximum page size of `getChildren`. */
export const MAX_CHILDREN = 1000;

export const listDrives = () => invoke<VolumeSummary[]>('list_drives');
export const pickFolder = () => invoke<string | null>('pick_folder');
export const listUserFolders = () => invoke<UserFolder[]>('user_folders');
/** Resolves to the scan id. `target` is a drive ("C:", "c:\\", "C") or an existing folder path. */
export const startScan = (target: string) => invoke<number>('start_scan', { target });
export const cancelScan = () => invoke<void>('cancel_scan');
export const getChildren = (node: number, sort: SortKey, offset: number, limit: number, scanId?: number) =>
  invoke<NodeView[]>('get_children', { node, sort, offset, limit, scanId });
export const getNode = (node: number) => invoke<NodeView>('get_node', { node });
export const getPath = (node: number) => invoke<string>('get_path', { node });
export interface FileFilter {
  name: string;
  extension: string;
  min_size: number;
  /** Only files of this category. */
  category?: FileCategory;
  /** Only files inside this node's subtree. */
  under?: number;
  /** `files` (default) or outermost `folders` sized by their whole subtree. */
  kind?: SearchKind;
  /** Exact names, case-insensitive. */
  names?: string[];
  /** Any-of extensions (files only). */
  extensions?: string[];
}

export type SearchKind = 'files' | 'folders';

export interface FileMatch {
  id: number;
  name: string;
  path: string;
  /** Total size for folders. */
  size: number;
  is_dir: boolean;
}

export interface SearchResults {
  total: number;
  /** Summed size of every match, not only the listed ones. */
  total_bytes: number;
  files: FileMatch[];
}

export interface CategoryTotal {
  category: FileCategory;
  bytes: number;
  files: number;
}

export interface ExtTotal {
  /** Lowercase, without the dot; empty for files without an extension. */
  ext: string;
  category: FileCategory;
  bytes: number;
  files: number;
}

/** File-type totals of a folder; `categories` holds all seven categories in a fixed order. */
export interface Breakdown {
  categories: CategoryTotal[];
  extensions: ExtTotal[];
  other_extensions: { bytes: number; files: number };
  /** Sizes recorded on the folders themselves; part of `total_bytes`, in no category. */
  folder_bytes: number;
  total_bytes: number;
  total_files: number;
}

export interface LocationStep {
  id: number;
  name: string;
  position: number | null;
}

export interface NodeDetails {
  node: NodeView;
  path: string;
  has_native_path: boolean;
}

export const searchFiles = (scanId: number, filter: FileFilter) =>
  invoke<SearchResults>('search_files', { scanId, filter });
/** File-type breakdown of a folder; `top` (1..50, default 10) limits the extension list. */
export const folderBreakdown = (scanId: number, node: number, top?: number) =>
  invoke<Breakdown>('folder_breakdown', { scanId, node, top });
export const locateNode = (scanId: number, node: number) =>
  invoke<LocationStep[]>('locate_node', { scanId, node });
export const nodeDetails = (scanId: number, node: number) =>
  invoke<NodeDetails>('node_details', { scanId, node });
export const copyNodePath = (scanId: number, node: number) =>
  invoke<void>('copy_node_path', { scanId, node });
export const revealNodeInFileManager = (scanId: number, node: number) =>
  invoke<void>('reveal_node_in_file_manager', { scanId, node });

/** What `moveNodeToTrash` removed from the tree. */
export interface TrashResult {
  bytes: number;
  files: number;
  dirs: number;
  /** Parent of the removed node. */
  parent: number;
}
/** Moves a node to the Recycle Bin / Trash; the path is resolved by the backend from the tree. */
export const moveNodeToTrash = (scanId: number, node: number) =>
  invoke<TrashResult>('move_node_to_trash', { scanId, node });

/** A node a batch moved to the Recycle Bin / Trash. */
export interface MovedItem {
  id: number;
  bytes: number;
  files: number;
  dirs: number;
  /** Parent of the removed node. */
  parent: number;
}

/** A node a batch could not move, with the reason. */
export interface FailedItem {
  id: number;
  name: string;
  error: string;
}

/** What `moveNodesToTrash` did; the totals cover the moved items only. */
export interface BatchTrashResult {
  moved: MovedItem[];
  failed: FailedItem[];
  bytes: number;
  files: number;
  dirs: number;
}

/** One entry of the Discard Pile as the backend sees it. */
export interface DiscardItem {
  id: number;
  name: string;
  path: string;
  /** Total size for folders. */
  size: number;
  is_dir: boolean;
}

/** The queued nodes that still exist; nested ones are folded into their ancestor. */
export interface DiscardSummary {
  items: DiscardItem[];
  count: number;
  bytes: number;
}

/** Most node ids the Discard Pile commands accept at once. */
export const MAX_DISCARD_IDS = 10_000;

/** Describes the queued nodes: outermost, still existing ones only. */
export const discardSummary = (scanId: number, nodes: number[]) =>
  invoke<DiscardSummary>('discard_summary', { scanId, nodes });
/** Moves the outermost of `nodes` to the Recycle Bin / Trash one by one; failures are reported per item. */
export const moveNodesToTrash = (scanId: number, nodes: number[]) =>
  invoke<BatchTrashResult>('move_nodes_to_trash', { scanId, nodes });

/**
 * Rectangles of the subtree under `node`, breadth-first (parents first). `header` (pixels)
 * reserves a name strip at the top of non-root directories at least 3 strips wide and tall.
 */
export const getTreemap = (node: number, width: number, height: number, header = 0) =>
  invoke<Rect[]>('get_treemap', { node, width, height, header });
/** Arcs of the subtree under `node`, ring by ring (inner rings first); `maxDepth` is 1..=12. */
export const getSunburst = (scanId: number, node: number, maxDepth?: number) =>
  invoke<SunburstArc[]>('get_sunburst', { scanId, node, maxDepth });
export const isElevated = () => invoke<boolean>('is_elevated');

/** User settings stored in `%APPDATA%\ByteForest\settings.json`. */
export interface Settings {
  always_elevated: boolean;
  /** Names, wildcards and absolute paths left out of every scan. */
  exclusions: string[];
  /** Automatic snapshots to keep (0 = off, at most 50). */
  auto_snapshot_keep: number;
}

/** Restarts the app as administrator (exits on success). Rejects with "declined" if UAC is refused. */
export const restartElevated = (target?: string) =>
  invoke<void>('restart_elevated', { target: target ?? null });
/** The `--scan` startup argument; resolves once, then null. */
export const takeStartupScan = () => invoke<string | null>('take_startup_scan');
export const getSettings = () => invoke<Settings>('get_settings');
export const setSettings = (settings: Settings) => invoke<void>('set_settings', { settings });

/** Saves a CSV or JSON report of a folder through a save dialog; the path, or null if cancelled. */
export const exportReport = (scanId: number, node: number) =>
  invoke<string | null>('export_report', { scanId, node });

/** Summary of a started comparison. */
export interface BaselineInfo {
  /** What was scanned when the snapshot was taken. */
  label: string;
  created_unix_ms: number;
  /** Net change of the whole scan since the snapshot, bytes. */
  total_delta: number;
  added: GoneInfo;
  removed: GoneInfo;
}

/** One row of the list of the biggest increases. */
export interface ChangeEntry {
  id: number;
  name: string;
  path: string;
  status: DiffStatus;
  base: number | null;
  current: number;
  delta: number;
}

/** Saves the scan through a save dialog; resolves to the file path, or null when cancelled. */
export const saveSnapshot = (scanId: number) => invoke<string | null>('save_snapshot', { scanId });
/** Picks a snapshot file and compares the scan with it; null when the dialog was cancelled. */
export const compareWithSnapshot = (scanId: number) =>
  invoke<BaselineInfo | null>('compare_with_snapshot', { scanId });
export const clearBaseline = () => invoke<void>('clear_baseline');
/** The nodes that grew or are new since the snapshot, largest first (`limit` is capped at 200). */
export const topChanges = (scanId: number, limit: number) =>
  invoke<ChangeEntry[]>('top_changes', { scanId, limit });

/** An automatic snapshot saved after a scan (stored in the app data folder). */
export interface AutoSnapshot {
  /** File name; the handle the other snapshot commands take. */
  id: string;
  /** What was scanned: "C:" or a folder path. */
  label: string;
  created_unix_ms: number;
  node_count: number;
  bytes_on_disk: number;
}

/** Where the shown result came from; a snapshot result is read-only. */
export type ResultSource = { kind: 'live' } | { kind: 'snapshot'; created_unix_ms: number };

/** What `openSnapshot` returns: the new current result, like `scan://done`. */
export interface OpenedSnapshot {
  scan_id: number;
  /** What was scanned when the snapshot was taken. */
  target: string;
  files: number;
  dirs: number;
  bytes: number;
  root: NodeView;
  source: { kind: 'snapshot'; created_unix_ms: number };
}

/** Saved automatic snapshots, newest first. */
export const listAutoSnapshots = () => invoke<AutoSnapshot[]>('list_auto_snapshots');
/** Makes the snapshot the current (read-only) result. Rejects while a scan is running. */
export const openSnapshot = (id: string) => invoke<OpenedSnapshot>('open_snapshot', { id });
/** Compares the live result with a saved snapshot. */
export const compareWithAutoSnapshot = (scanId: number, id: string) =>
  invoke<BaselineInfo>('compare_with_auto_snapshot', { scanId, id });
export const deleteAutoSnapshot = (id: string) => invoke<void>('delete_auto_snapshot', { id });
/** Fires after automatic snapshots were added, pruned or deleted. */
export const onSnapshotsChanged = (cb: () => void): Promise<UnlistenFn> =>
  listen('snapshots-changed', () => cb());

export const onScanProgress = (cb: (p: ScanProgress) => void): Promise<UnlistenFn> =>
  listen<ScanProgress>('scan://progress', (e) => cb(e.payload));
export const onScanDone = (cb: (d: ScanDone) => void): Promise<UnlistenFn> =>
  listen<ScanDone>('scan://done', (e) => cb(e.payload));
export const onScanError = (cb: (e: ScanErrorEvent) => void): Promise<UnlistenFn> =>
  listen<ScanErrorEvent>('scan://error', (e) => cb(e.payload));
