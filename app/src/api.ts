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
}

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

export interface ScanDone {
  scan_id: number;
  stats: ScanStats;
  root: NodeView;
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
}

export interface FileMatch {
  id: number;
  name: string;
  path: string;
  size: number;
}

export interface SearchResults {
  total: number;
  files: FileMatch[];
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

/**
 * Rectangles of the subtree under `node`, breadth-first (parents first). `header` (pixels)
 * reserves a name strip at the top of non-root directories at least 3 strips wide and tall.
 */
export const getTreemap = (node: number, width: number, height: number, header = 0) =>
  invoke<Rect[]>('get_treemap', { node, width, height, header });
export const isElevated = () => invoke<boolean>('is_elevated');

/** User settings stored in `%APPDATA%\ByteForest\settings.json`. */
export interface Settings {
  always_elevated: boolean;
}

/** Restarts the app as administrator (exits on success). Rejects with "declined" if UAC is refused. */
export const restartElevated = (target?: string) =>
  invoke<void>('restart_elevated', { target: target ?? null });
/** The `--scan` startup argument; resolves once, then null. */
export const takeStartupScan = () => invoke<string | null>('take_startup_scan');
export const getSettings = () => invoke<Settings>('get_settings');
export const setSettings = (settings: Settings) => invoke<void>('set_settings', { settings });

export const onScanProgress = (cb: (p: ScanProgress) => void): Promise<UnlistenFn> =>
  listen<ScanProgress>('scan://progress', (e) => cb(e.payload));
export const onScanDone = (cb: (d: ScanDone) => void): Promise<UnlistenFn> =>
  listen<ScanDone>('scan://done', (e) => cb(e.payload));
export const onScanError = (cb: (e: ScanErrorEvent) => void): Promise<UnlistenFn> =>
  listen<ScanErrorEvent>('scan://error', (e) => cb(e.payload));
