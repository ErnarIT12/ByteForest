/** The list of automatic snapshots saved on this computer, kept fresh for the sidebar. */
import { create } from 'zustand';
import { listAutoSnapshots, onSnapshotsChanged, type AutoSnapshot } from './api';

interface AutoSnapshotsState {
  list: AutoSnapshot[];
  /** Reloads the list; a failure keeps the previous one (the feature is a convenience). */
  refresh(): Promise<void>;
}

/** Ensures out-of-order `listAutoSnapshots` responses cannot overwrite a newer list. */
let refreshSeq = 0;

export const useAutoSnapshots = create<AutoSnapshotsState>()((set) => ({
  list: [],
  refresh: async () => {
    const seq = ++refreshSeq;
    try {
      const list = await listAutoSnapshots();
      if (seq === refreshSeq) set({ list });
    } catch {
      /* Keep what is shown. */
    }
  },
}));

/** Loads the list now and again whenever the back end says it changed; returns the cleanup. */
export function watchAutoSnapshots(): () => void {
  let active = true;
  const refresh = () => {
    if (active) void useAutoSnapshots.getState().refresh();
  };
  refresh();
  const subscription = onSnapshotsChanged(refresh).catch(() => () => {});
  return () => {
    active = false;
    void subscription.then((unlisten) => unlisten());
  };
}
