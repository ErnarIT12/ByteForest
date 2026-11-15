/**
 * Application state (zustand): the scan lifecycle, the shown tree and the
 * focus / selection shared by the folder tree, the treemap and the breadcrumbs.
 *
 * Actions here never call IPC; `scanController.ts` does that and feeds results in.
 */
import { create } from 'zustand';
import {
  SCAN_CANCELLED,
  type NodeView,
  type ScanDone,
  type ScanErrorEvent,
  type ScanProgress,
  type ScanStats,
  type VolumeSummary,
} from './api';
import { strings } from './strings';

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
}

interface AppState {
  drives: VolumeSummary[] | null;
  drivesError: string | null;
  /** Drive letter picked in the top bar. */
  target: string | null;
  elevated: boolean | null;

  scanning: boolean;
  /** Drive letter of the running scan. */
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
  /** Drive letter of the shown result. */
  resultTarget: string | null;
  /** Bumped for every new result so views drop their caches. */
  generation: number;

  focusId: number | null;
  focusPath: Crumb[];
  selectedId: number | null;
  reveal: RevealRequest | null;
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
  generation: 0,
  focusId: null,
  focusPath: [],
  selectedId: null,
  reveal: null,
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
      target: s.target ?? (drives.find((d) => d.letter === 'C') ?? drives[0])?.letter ?? null,
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
      generation: s.generation + 1,
      focusId: d.root.id,
      focusPath: [{ id: d.root.id, name: d.root.name }],
      selectedId: null,
      reveal: null,
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

  dismissError: () => set({ error: null, errorTitle: null }),
  setElevating: (elevating) => set({ elevating }),
  elevationFailed: (message) =>
    set({ error: message, errorTitle: strings.elevateFailed, elevating: false }),
  dismissElevationBar: () => set({ elevationBarDismissed: true }),
  reset: () => set(initialState),
}));
