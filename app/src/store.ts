/**
 * Application state (zustand): the scan lifecycle, the shown tree and the
 * focus / selection shared by the folder tree, the treemap and the breadcrumbs.
 *
 * Actions here never call IPC; `scanController.ts` does that and feeds results in.
 */
import { create } from 'zustand';
import {
  SCAN_CANCELLED,
  type BaselineInfo,
  type BatchTrashResult,
  type FileCategory,
  type LocationStep,
  type NodeView,
  type OpenedSnapshot,
  type ResultSource,
  type ExcludedInfo,
  type ScanDone,
  type ScanErrorEvent,
  type ScanProgress,
  type ScanStats,
  type TrashResult,
  type UserFolder,
  type VolumeSummary,
} from './api';
import { survivingSelection } from './discardModel';
import { loadMapView, saveMapView, type MapView } from './mapView';
import { addRecentScan, loadRecentScans, saveRecentScans, type RecentScan } from './recentScans';
import { locationOf } from './scanTargets';
import { orderUserFolders } from './sidebarModel';
import { strings } from './strings';

/** Asks the folder tree to re-read the pages of `parentId` and its ancestors after a removal. */
export interface TreeInvalidation {
  seq: number;
  /** The removed node. */
  nodeId: number;
  parentId: number;
  /** A batch removal: refresh every loaded folder, not only the chain of one parent. */
  all?: boolean;
}

/** One entry of the focus path (root first). */
export interface Crumb {
  id: number;
  name: string;
}

/**
 * Which scan ids the UI listens to. Events of a new scan can arrive before `startScan`
 * resolves with its id, so while `pending` any id newer than `lastKnown` is accepted;
 * once resolved, only `current` is.
 */
export interface ScanGate {
  pending: boolean;
  current: number | null;
  /** Highest scan id seen so far (resolved or finished). */
  lastKnown: number;
}

/** Whether an event tagged `scanId` belongs to the scan the UI is waiting for. */
export function acceptEvent(gate: ScanGate, scanId: number): boolean {
  if (gate.pending) return scanId > gate.lastKnown;
  return gate.current !== null && scanId === gate.current;
}

/** A request for the folder tree to expand `path` (root first) and scroll to its end. */
export interface RevealRequest {
  path: number[];
  seq: number;
  positions?: (number | null)[];
  filename?: string;
  /** Switch the explorer back to the folder tree (the request came from outside it). */
  showTree?: boolean;
}

/** Pre-filled search (opened from a folder's file-type breakdown). */
export interface SearchPreset {
  /** Bumped for every request so the same preset can be applied again. */
  seq: number;
  category?: FileCategory;
  extension?: string;
  /** Folder the search is limited to. */
  under: { id: number; name: string };
}

interface AppState {
  drives: VolumeSummary[] | null;
  drivesError: string | null;
  /** Desktop, Downloads and Documents of the user, in display order. */
  userFolders: UserFolder[];
  /** Finished scans, newest first (persisted). */
  recentScans: RecentScan[];
  /** Disk map shown in the explorer (persisted). */
  largeFilesOpen: boolean;
  mapView: MapView;
  /** Location picked in the sidebar: a drive letter ("C") or a folder path. */
  target: string | null;
  elevated: boolean | null;
  isMac: boolean;

  scanning: boolean;
  /** Exact target of the running scan: "C:" or a folder path. */
  scanTarget: string | null;
  scanStartedAt: number | null;
  progress: ScanProgress | null;
  gate: ScanGate;
  /** Id of the last scan that finished (done or error). */
  lastFinished: number;
  /** How the last scan ended, for screen reader announcements. */
  lastOutcome: 'done' | 'cancelled' | 'failed' | null;

  /** Result being shown. */
  stats: ScanStats | null;
  /** What the exclusion settings cut out of the shown result, if anything. */
  excluded: ExcludedInfo | null;
  root: NodeView | null;
  rootId: number | null;
  /** Exact target of the shown result: "C:" or a folder path. */
  resultTarget: string | null;
  resultScanId: number | null;
  /** Where the shown result came from; a snapshot is read-only. */
  resultSource: ResultSource;
  /** Bumped for every new result so views drop their caches. */
  generation: number;
  /** Snapshot the result is being compared with, or null. */
  baseline: BaselineInfo | null;
  /** Bumped whenever `baseline` is set or cleared, so the folder tree refetches its loaded pages in place. */
  baselineSeq: number;

  focusId: number | null;
  focusPath: Crumb[];
  selectedId: number | null;
  reveal: RevealRequest | null;
  /** Last removal; the folder tree refreshes its cached pages when `seq` changes. */
  invalidation: TreeInvalidation | null;
  /** Latest request to open the search pane with these filters. */
  searchPreset: SearchPreset | null;
  error: string | null;
  /** Heading of the error toast; null means the default scan failure title. */
  errorTitle: string | null;
  /** Waiting for the UAC prompt of a restart as administrator. */
  elevating: boolean;
  /** The post-scan elevation hint was closed (reset by the next scan). */
  elevationBarDismissed: boolean;
}

interface AppActions {
  setDrives(drives: VolumeSummary[]): void;
  setDrivesError(message: string): void;
  setTarget(letter: string): void;
  setUserFolders(folders: UserFolder[]): void;
  setMapView(view: MapView): void;
  setElevated(elevated: boolean): void;

  /** `target` is exactly what is passed to `startScan` ("C:" or a folder path). */
  beginScan(target: string): void;
  scanResolved(scanId: number): void;
  /** A `startScan` call that was superseded by a newer one still tells us its id. */
  noteScanId(scanId: number): void;
  scanFailedToStart(message: string): void;
  handleProgress(p: ScanProgress): void;
  handleDone(d: ScanDone): void;
  handleError(e: ScanErrorEvent): void;
  /** Shows a snapshot opened from disk as the current (read-only) result. */
  handleOpened(o: OpenedSnapshot): void;

  /** Zooms to directory `id`. `path` (root..id) is needed unless `id` is already on the path. */
  setFocus(id: number, path?: Crumb[]): void;
  select(id: number | null): void;
  goUp(): void;
  /** Selects `path`'s last node and asks the tree to expand the path to it. */
  revealNode(path: number[]): void;
  revealSearchResult(scanId: number, steps: LocationStep[], opts?: { showTree?: boolean }): void;
  /** Applies a successful move to the trash: totals, focus, selection and view refresh. */
  applyRemoval(id: number, result: TrashResult): void;
  /**
   * Applies a batch move to the trash in one pass. `selectionChain` is the path root..selection
   * from before the move; it finds the surviving ancestor when the selection was inside a
   * removed folder.
   */
  applyBatchRemoval(result: BatchTrashResult, selectionChain?: number[] | null): void;
  /** Starts (info) or stops (null) comparing; views refetch to pick up the changes. */
  setBaseline(info: BaselineInfo | null): void;
  /** Asks the explorer to show the file search with the filters of `preset`. */
  openSearch(preset: Omit<SearchPreset, 'seq'>): void;
  dismissError(): void;
  setElevating(elevating: boolean): void;
  /** Shows a non-scan failure of the restart as administrator in the toast. */
  elevationFailed(message: string): void;
  dismissElevationBar(): void;
  /** Back to the initial state (tests). */
  reset(): void;
}

export type AppStore = AppState & AppActions;

const initialState: AppState = {
  drives: null,
  drivesError: null,
  userFolders: [],
  recentScans: loadRecentScans(),
  mapView: loadMapView(),
  target: null,
  elevated: null,
  isMac: typeof navigator !== 'undefined' && /Macintosh/i.test(navigator.userAgent),
  scanning: false,
  scanTarget: null,
  scanStartedAt: null,
  progress: null,
  gate: { pending: false, current: null, lastKnown: 0 },
  lastFinished: 0,
  lastOutcome: null,
  stats: null,
  excluded: null,
  root: null,
  rootId: null,
  resultTarget: null,
  resultScanId: null,
  resultSource: { kind: 'live' },
  generation: 0,
  baseline: null,
  baselineSeq: 0,
  focusId: null,
  focusPath: [],
  selectedId: null,
  reveal: null,
  invalidation: null,
  largeFilesOpen: false,
  searchPreset: null,
  error: null,
  errorTitle: null,
  elevating: false,
  elevationBarDismissed: false,
};

/** Gate after scan `id` finished: no longer pending on it, and it is the newest known id. */
function finishedGate(gate: ScanGate, id: number): ScanGate {
  return { ...gate, lastKnown: Math.max(gate.lastKnown, id) };
}

/**
 * The state patch shared by single and batch removals: smaller totals, a focus that leaves
 * removed folders, and a bumped generation so views drop their caches.
 */
function shrink(
  s: AppState,
  totals: { bytes: number; files: number; dirs: number },
  removed: ReadonlySet<number>,
): Partial<AppState> {
  const minus = (value: number, by: number) => Math.max(0, value - by);
  const root = s.root && {
    ...s.root,
    size: minus(s.root.size, totals.bytes),
    files: minus(s.root.files, totals.files),
    dirs: minus(s.root.dirs, totals.dirs),
  };
  const stats = s.stats && {
    ...s.stats,
    bytes: minus(s.stats.bytes, totals.bytes),
    files: minus(s.stats.files, totals.files),
    dirs: minus(s.stats.dirs, totals.dirs),
  };
  // A removed folder on (or above) the focus moves the focus to its parent.
  const at = s.focusPath.findIndex((c, i) => i > 0 && removed.has(c.id));
  const focusPath = at > 0 ? s.focusPath.slice(0, at) : s.focusPath;
  const focusMoved = focusPath !== s.focusPath;
  return {
    root,
    stats,
    // The scan got smaller by what was removed, so the net change since the snapshot did too.
    baseline: s.baseline && { ...s.baseline, total_delta: s.baseline.total_delta - totals.bytes },
    generation: s.generation + 1,
    focusPath,
    focusId: focusPath[focusPath.length - 1]?.id ?? s.focusId,
    reveal: focusMoved
      ? { path: focusPath.map((c) => c.id), seq: (s.reveal?.seq ?? 0) + 1 }
      : s.reveal,
  };
}

export const useAppStore = create<AppStore>()((set, get) => ({
  ...initialState,

  setDrives: (drives) =>
    set((s) => ({
      drives,
      drivesError: null,
      target: s.target ?? drives.find((d) => d.letter === 'C')?.letter ?? drives[0]?.path ?? drives[0]?.letter ?? null,
    })),
  setDrivesError: (message) => set({ drivesError: message, drives: [] }),
  setTarget: (letter) => set({ target: letter }),
  setUserFolders: (folders) => set({ userFolders: orderUserFolders(folders) }),
  setMapView: (view) => {
    saveMapView(view);
    set({ mapView: view });
  },
  setElevated: (elevated) => set({ elevated }),

  beginScan: (target) =>
    set((s) => ({
      scanning: true,
      scanTarget: target,
      scanStartedAt: Date.now(),
      progress: null,
      error: null,
      errorTitle: null,
      elevationBarDismissed: false,
      // The back end drops the comparison when a scan starts.
      baseline: null,
      gate: { ...s.gate, pending: true },
    })),

  scanResolved: (scanId) =>
    set((s) => ({
      gate: { pending: false, current: scanId, lastKnown: Math.max(s.gate.lastKnown, scanId) },
      // The outcome may already have arrived while the call was pending.
      scanning: s.scanning && s.lastFinished !== scanId,
    })),

  noteScanId: (scanId) =>
    set((s) => ({ gate: { ...s.gate, lastKnown: Math.max(s.gate.lastKnown, scanId) } })),

  scanFailedToStart: (message) =>
    set((s) => ({
      scanning: false,
      progress: null,
      error: message,
      errorTitle: null,
      lastOutcome: 'failed',
      gate: { ...s.gate, pending: false },
    })),

  handleProgress: (p) => {
    const s = get();
    if (!s.scanning || !acceptEvent(s.gate, p.scan_id)) return;
    set({ progress: p });
  },

  handleDone: (d) => {
    const s = get();
    if (!acceptEvent(s.gate, d.scan_id)) return;
    const recentScans = s.scanTarget
      ? addRecentScan(s.recentScans, {
          target: s.scanTarget,
          label: d.root.name,
          finishedAt: Date.now(),
          bytes: d.root.size,
        })
      : s.recentScans;
    if (recentScans !== s.recentScans) saveRecentScans(recentScans);
    set({
      recentScans,
      scanning: false,
      progress: null,
      gate: finishedGate(s.gate, d.scan_id),
      lastFinished: d.scan_id,
      lastOutcome: 'done',
      stats: d.stats,
      excluded: d.excluded && d.excluded.items > 0 ? d.excluded : null,
      root: d.root,
      rootId: d.root.id,
      resultTarget: s.scanTarget,
      resultScanId: d.scan_id,
      resultSource: { kind: 'live' },
      generation: s.generation + 1,
      baseline: null,
      baselineSeq: s.baselineSeq + 1,
      focusId: d.root.id,
      focusPath: [{ id: d.root.id, name: d.root.name }],
      selectedId: null,
      reveal: null,
      invalidation: null,
      largeFilesOpen: false,
  searchPreset: null,
    });
  },

  handleError: (e) => {
    const s = get();
    if (!acceptEvent(s.gate, e.scan_id)) return;
    set({
      scanning: false,
      progress: null,
      gate: finishedGate(s.gate, e.scan_id),
      lastFinished: e.scan_id,
      lastOutcome: e.message === SCAN_CANCELLED ? 'cancelled' : 'failed',
      error: e.message === SCAN_CANCELLED ? null : e.message,
      errorTitle: null,
    });
  },

  handleOpened: (o) => {
    const s = get();
    set({
      scanning: false,
      progress: null,
      error: null,
      errorTitle: null,
      gate: { pending: false, current: o.scan_id, lastKnown: Math.max(s.gate.lastKnown, o.scan_id) },
      lastFinished: o.scan_id,
      lastOutcome: 'done',
      stats: {
        files: o.files,
        dirs: o.dirs,
        bytes: o.bytes,
        elapsed_ms: 0,
        method: 'Walk',
        fallback_reason: null,
        skipped_records: 0,
        errors: 0,
      },
      excluded: null,
      root: o.root,
      rootId: o.root.id,
      resultTarget: o.target,
      resultScanId: o.scan_id,
      resultSource: o.source,
      // "Scan" in the top bar then rescans what the snapshot shows.
      target: locationOf(o.target),
      generation: s.generation + 1,
      baseline: null,
      baselineSeq: s.baselineSeq + 1,
      focusId: o.root.id,
      focusPath: [{ id: o.root.id, name: o.root.name }],
      selectedId: null,
      reveal: null,
      invalidation: null,
      largeFilesOpen: false,
  searchPreset: null,
    });
  },

  setFocus: (id, path) => {
    const s = get();
    let focusPath: Crumb[];
    if (path && path.length > 0 && path[path.length - 1].id === id) {
      focusPath = path;
    } else {
      const at = s.focusPath.findIndex((c) => c.id === id);
      if (at < 0) return;
      focusPath = s.focusPath.slice(0, at + 1);
    }
    set({
      focusId: id,
      focusPath,
      selectedId: null,
      reveal: { path: focusPath.map((c) => c.id), seq: (s.reveal?.seq ?? 0) + 1 },
    });
  },

  select: (id) => set({ selectedId: id }),

  goUp: () => {
    const s = get();
    if (s.focusPath.length <= 1) return;
    const left = s.focusPath[s.focusPath.length - 1];
    const focusPath = s.focusPath.slice(0, -1);
    set({ focusId: focusPath[focusPath.length - 1].id, focusPath, selectedId: left.id });
  },

  revealNode: (path) => {
    if (path.length === 0) return;
    set((s) => ({
      selectedId: path[path.length - 1],
      reveal: { path, seq: (s.reveal?.seq ?? 0) + 1 },
    }));
  },

  revealSearchResult: (scanId, steps, opts) => {
    const s = get();
    if (s.resultScanId !== scanId || steps.length < 2) return;
    const file = steps[steps.length - 1];
    const focusPath = steps.slice(0, -1).map(({ id, name }) => ({ id, name }));
    set({
      largeFilesOpen: false,
      selectedId: file.id,
      focusId: focusPath[focusPath.length - 1].id,
      focusPath,
      reveal: {
        path: steps.map((step) => step.id),
        positions: steps.map((step) => step.position),
        filename: file.name,
        showTree: opts?.showTree,
        seq: (s.reveal?.seq ?? 0) + 1,
      },
    });
  },

  applyRemoval: (id, result) => {
    const s = get();
    if (!s.root) return;
    set({
      ...shrink(s, result, new Set([id])),
      selectedId: result.parent,
      invalidation: { seq: (s.invalidation?.seq ?? 0) + 1, nodeId: id, parentId: result.parent },
    });
  },

  applyBatchRemoval: (result, selectionChain = null) => {
    const s = get();
    if (!s.root || result.moved.length === 0) return;
    const removed = new Set(result.moved.map((m) => m.id));
    const direct = result.moved.find((m) => m.id === s.selectedId);
    const selectedId = direct
      ? direct.parent
      : (survivingSelection(selectionChain, removed) ?? s.selectedId);
    const first = result.moved[0];
    set({
      ...shrink(s, result, removed),
      selectedId,
      invalidation: {
        seq: (s.invalidation?.seq ?? 0) + 1,
        nodeId: first.id,
        parentId: first.parent,
        all: true,
      },
    });
  },

  setBaseline: (info) =>
    set((s) => ({ baseline: info, baselineSeq: s.baselineSeq + 1, generation: s.generation + 1 })),

  openSearch: (preset) =>
    set((s) => ({ largeFilesOpen: true, searchPreset: { ...preset, seq: (s.searchPreset?.seq ?? 0) + 1 } })),
  dismissError: () => set({ error: null, errorTitle: null }),
  setElevating: (elevating) => set({ elevating }),
  elevationFailed: (message) =>
    set({ error: message, errorTitle: strings.elevateFailed, elevating: false }),
  dismissElevationBar: () => set({ elevationBarDismissed: true }),
  reset: () => set(initialState),
}));
