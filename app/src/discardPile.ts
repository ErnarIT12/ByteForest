/**
 * The Discard Pile: nodes queued for a batch move to the Recycle Bin / Trash. The queue only
 * holds node ids of the shown scan result; the backend describes them (`discardSummary`) and
 * does the move (`moveNodesToTrash`). Queuing deletes nothing.
 */
import { create } from 'zustand';
import {
  discardSummary,
  locateNode,
  moveNodesToTrash,
  type DiscardSummary,
  type FailedItem,
} from './api';
import { addIds, pruneIds, removeId, removeIds } from './discardModel';
import { canChangeDisk } from './snapshotsModel';
import { useAppStore } from './store';

/** How the last batch move ended. */
export interface DiscardOutcome {
  moved: number;
  bytes: number;
  failed: FailedItem[];
  /** Set when the whole batch could not run. */
  error: string | null;
}

interface DiscardState {
  /** Scan result the queue belongs to; null while there is none (or a scan runs). */
  scanId: number | null;
  ids: readonly number[];
  idSet: ReadonlySet<number>;
  /** Backend description of the queue; null until the first answer. */
  summary: DiscardSummary | null;
  confirming: boolean;
  busy: boolean;
  outcome: DiscardOutcome | null;

  has(id: number): boolean;
  add(ids: readonly number[]): void;
  remove(id: number): void;
  /** Removes `toRemove` and adds `toAdd` in a single batch refresh. */
  updateBatch(toAdd: readonly number[], toRemove: readonly number[]): void;
  clear(): void;
  /** Ties the queue to a scan result; a different one starts an empty queue. */
  bindScan(scanId: number | null): void;
  /** Asks the backend which queued items still exist and prunes the others. */
  refresh(): Promise<void>;
  requestMoveAll(): void;
  cancelMoveAll(): void;
  confirmMoveAll(): Promise<void>;
  dismissOutcome(): void;
}

const initial = {
  scanId: null,
  ids: [] as readonly number[],
  idSet: new Set<number>() as ReadonlySet<number>,
  summary: null,
  confirming: false,
  busy: false,
  outcome: null,
};

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

/** Newest `refresh` wins; older answers are dropped. */
let refreshSeq = 0;

export const useDiscardPile = create<DiscardState>()((set, get) => {
  const setIds = (ids: readonly number[]) => {
    if (ids === get().ids) return;
    set({ ids, idSet: new Set(ids) });
    void get().refresh();
  };

  return {
    ...initial,

    has: (id) => get().idSet.has(id),

    add: (ids) => {
      if (get().scanId === null || get().busy) return;
      // A snapshot view is read-only: nothing in it may be queued for deletion.
      if (!canChangeDisk(useAppStore.getState().resultSource)) return;
      setIds(addIds(get().ids, ids, useAppStore.getState().rootId));
    },

    remove: (id) => {
      if (get().busy) return;
      setIds(removeId(get().ids, id));
    },

    updateBatch: (toAdd, toRemove) => {
      if (get().scanId === null || get().busy) return;
      let next = toRemove.length > 0 ? removeIds(get().ids, new Set(toRemove)) : get().ids;
      if (toAdd.length > 0 && canChangeDisk(useAppStore.getState().resultSource)) {
        next = addIds(next, toAdd, useAppStore.getState().rootId);
      }
      setIds(next);
    },

    clear: () => {
      if (get().busy) return;
      refreshSeq += 1;
      set({ ids: [], idSet: new Set(), summary: null, confirming: false, outcome: null });
    },

    bindScan: (scanId) => {
      if (get().scanId === scanId) return;
      refreshSeq += 1;
      set({ ...initial, idSet: new Set(), scanId });
    },

    refresh: async () => {
      const { scanId, ids } = get();
      const seq = ++refreshSeq;
      if (scanId === null || ids.length === 0) {
        set({ summary: null });
        return;
      }
      try {
        const summary = await discardSummary(scanId, [...ids]);
        if (seq !== refreshSeq || get().scanId !== scanId) return;
        const pruned = pruneIds(get().ids, summary);
        set({ summary, ids: pruned, idSet: pruned === get().ids ? get().idSet : new Set(pruned) });
      } catch {
        // The scan changed under us; `bindScan` resets the queue.
      }
    },

    requestMoveAll: () => {
      const { summary, busy } = get();
      if (!busy && summary && summary.count > 0) set({ confirming: true, outcome: null });
    },

    cancelMoveAll: () => {
      if (!get().busy) set({ confirming: false });
    },

    confirmMoveAll: async () => {
      const { scanId, summary, busy } = get();
      if (scanId === null || !summary || summary.count === 0 || busy) return;
      set({ busy: true });
      const nodes = summary.items.map((item) => item.id);
      // The path root..selection, to find a surviving ancestor if the selection disappears.
      const selected = useAppStore.getState().selectedId;
      let chain: number[] | null = null;
      if (selected !== null) {
        try {
          chain = (await locateNode(scanId, selected)).map((step) => step.id);
        } catch {
          chain = null;
        }
      }
      let outcome: DiscardOutcome;
      try {
        const result = await moveNodesToTrash(scanId, nodes);
        // One pass over the UI state, even for hundreds of items.
        if (useAppStore.getState().resultScanId === scanId) {
          useAppStore.getState().applyBatchRemoval(result, chain);
        }
        if (get().scanId === scanId) {
          const gone = new Set(result.moved.map((m) => m.id));
          const rest = removeIds(get().ids, gone);
          set({ ids: rest, idSet: new Set(rest) });
        }
        outcome = {
          moved: result.moved.length,
          bytes: result.bytes,
          failed: result.failed,
          error: null,
        };
      } catch (error) {
        outcome = { moved: 0, bytes: 0, failed: [], error: message(error) };
      }
      set({ busy: false, confirming: false, outcome });
      void get().refresh();
    },

    dismissOutcome: () => set({ outcome: null }),
  };
});

/** Adds `id` to the pile, or removes it when it is queued already. */
export function toggleQueued(id: number): void {
  const pile = useDiscardPile.getState();
  if (pile.has(id)) pile.remove(id);
  else pile.add([id]);
}

/** Selects a queued item in the folder tree and the map (like opening a search result). */
export async function revealQueued(id: number): Promise<void> {
  const scanId = useDiscardPile.getState().scanId;
  if (scanId === null) return;
  try {
    const steps = await locateNode(scanId, id);
    useAppStore.getState().revealSearchResult(scanId, steps, { showTree: true });
  } catch {
    // The item is gone; the next refresh drops it from the pile.
    void useDiscardPile.getState().refresh();
  }
}
