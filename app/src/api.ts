/**
 * Typed wrappers over the Tauri IPC surface of ByteForest.
 *
 * Argument objects use camelCase keys (Tauri converts them to the Rust snake_case names):
 * `node`, `sort`, `offset`, `limit`, `width`, `height`, `target`.
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
  letter: string;
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

export interface Rect {
  id: number;
  x: number;
  y: number;
  w: number;
  h: number;
  depth: number;
  is_dir: boolean;
}

export interface ScanProgress {
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
  stats: ScanStats;
  root: NodeView;
}

export interface ScanErrorEvent {
  message: string;
}

/** Maximum page size of `getChildren`. */
export const MAX_CHILDREN = 1000;

export const listDrives = () => invoke<VolumeSummary[]>('list_drives');
/** `target` is a drive ("C:", "c:\\", "C") or an existing folder path. */
export const startScan = (target: string) => invoke<void>('start_scan', { target });
export const cancelScan = () => invoke<void>('cancel_scan');
export const getChildren = (node: number, sort: SortKey, offset: number, limit: number) =>
  invoke<NodeView[]>('get_children', { node, sort, offset, limit });
export const getNode = (node: number) => invoke<NodeView>('get_node', { node });
export const getPath = (node: number) => invoke<string>('get_path', { node });
export const getTreemap = (node: number, width: number, height: number) =>
  invoke<Rect[]>('get_treemap', { node, width, height });
export const isElevated = () => invoke<boolean>('is_elevated');

export const onScanProgress = (cb: (p: ScanProgress) => void): Promise<UnlistenFn> =>
  listen<ScanProgress>('scan://progress', (e) => cb(e.payload));
export const onScanDone = (cb: (d: ScanDone) => void): Promise<UnlistenFn> =>
  listen<ScanDone>('scan://done', (e) => cb(e.payload));
export const onScanError = (cb: (e: ScanErrorEvent) => void): Promise<UnlistenFn> =>
  listen<ScanErrorEvent>('scan://error', (e) => cb(e.payload));
