/**
 * Application state (zustand): the scan lifecycle, the shown tree and the
 * focus / selection shared by the folder tree, the treemap and the breadcrumbs.
 *
 * Actions here never call IPC; `scanController.ts` does that and feeds results in.
 */
import { create } from 'zustand';
import {
  SCAN_CANCELLED,
  type LocationStep,
  type NodeView,
  type ScanDone,
  type ScanErrorEvent,
  type ScanProgress,
  type ScanStats,
  type TrashResult,
  type VolumeSummary,
} from './api';
import { strings } from './strings';

/** Asks the folder tree to re-read the pages of `parentId` and its ancestors after a removal. */
export interface TreeInvalidation {
  seq: number;
  /** The removed node. */
  nodeId: number;
  parentId: number;
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
}

interface AppState {
  drives: VolumeSummary[] | null;
  drivesError: string | null;
  /** Drive letter picked in the top bar. */
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
  root: NodeView | null;
  rootId: number | null;
  /** Exact target of the shown result: "C:" or a folder path. */
  resultTarget: string | null;
  resultScanId: number | null;
  /** Bumped for every new result so views drop their caches. */
  generation: number;

  focusId: number | null;
  focusPath: Crumb[];
  selectedId: number | null;
  reveal: RevealRequest | null;
  /** Last removal; the folder tree refreshes its cached pages when `seq` changes. */
  invalidation: TreeInvalidation | null;
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

  /** Zooms to directory `id`. `path` (root..id) is needed unless `id` is already on the path. */
  setFocus(id: number, path?: Crumb[]): void;
  select(id: number | null): void;
  goUp(): void;
  /** Selects `path`'s last node and asks the tree to expand the path to it. */
  revealNode(path: number[]): void;
  revealSearchResult(scanId: number, steps: LocationStep[]): void;
  /** Applies a successful move to the trash: totals, focus, selection and view refresh. */
  applyRemoval(id: number, result: TrashResult): void;
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
  root: null,
  rootId: null,
  resultTarget: null,
  resultScanId: null,
  generation: 0,
  focusId: null,
  focusPath: [],
  selectedId: null,
  reveal: null,
  invalidation: null,
  error: null,
  errorTitle: null,
  elevating: false,
  elevationBarDismissed: false,
};

/** Gate after scan `id` finished: no longer pending on it, and it is the newest known id. */
function finishedGate(gate: ScanGate, id: number): ScanGate {
  return { ...gate, lastKnown: Math.max(gate.lastKnown, id) };
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
    set({
      scanning: false,
      progress: null,
      gate: finishedGate(s.gate, d.scan_id),
      lastFinished: d.scan_id,
      lastOutcome: 'done',
      stats: d.stats,
      root: d.root,
      rootId: d.root.id,
      resultTarget: s.scanTarget,
      resultScanId: d.scan_id,
      generation: s.generation + 1,
      focusId: d.root.id,
      focusPath: [{ id: d.root.id, name: d.root.name }],
      selectedId: null,
      reveal: null,
      invalidation: null,
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

  revealSearchResult: (scanId, steps) => {
    const s = get();
    if (s.resultScanId !== scanId || steps.length < 2) return;
    const file = steps[steps.length - 1];
    const focusPath = steps.slice(0, -1).map(({ id, name }) => ({ id, name }));
    set({
      selectedId: file.id,
      focusId: focusPath[focusPath.length - 1].id,
      focusPath,
      reveal: {
        path: steps.map((step) => step.id),
        positions: steps.map((step) => step.position),
        filename: file.name,
        seq: (s.reveal?.seq ?? 0) + 1,
      },
    });
  },

  applyRemoval: (id, result) => {
    const s = get();
    if (!s.root) return;
    const minus = (value: number, by: number) => Math.max(0, value - by);
    const root = {
      ...s.root,
      size: minus(s.root.size, result.bytes),
      files: minus(s.root.files, result.files),
      dirs: minus(s.root.dirs, result.dirs),
    };
    const stats = s.stats && {
      ...s.stats,
      bytes: minus(s.stats.bytes, result.bytes),
      files: minus(s.stats.files, result.files),
      dirs: minus(s.stats.dirs, result.dirs),
    };
    // A removed folder on (or above) the focus moves the focus to its parent.
    const at = s.focusPath.findIndex((c) => c.id === id);
    const focusPath = at > 0 ? s.focusPath.slice(0, at) : s.focusPath;
    const focusMoved = focusPath !== s.focusPath;
    set({
      root,
      stats,
      generation: s.generation + 1,
      focusPath,
      focusId: focusPath[focusPath.length - 1]?.id ?? s.focusId,
      selectedId: result.parent,
      reveal: focusMoved
        ? { path: focusPath.map((c) => c.id), seq: (s.reveal?.seq ?? 0) + 1 }
        : s.reveal,
      invalidation: { seq: (s.invalidation?.seq ?? 0) + 1, nodeId: id, parentId: result.parent },
    });
  },

  dismissError: () => set({ error: null, errorTitle: null }),
  setElevating: (elevating) => set({ elevating }),
  elevationFailed: (message) =>
    set({ error: message, errorTitle: strings.elevateFailed, elevating: false }),
  dismissElevationBar: () => set({ elevationBarDismissed: true }),
  reset: () => set(initialState),
}));
