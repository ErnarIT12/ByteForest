/** The list of automatic snapshots saved on this computer, kept fresh for the sidebar. */
import { create } from 'zustand';
import { listAutoSnapshots, onSnapshotsChanged, type AutoSnapshot } from './api';

interface AutoSnapshotsState {
  list: AutoSnapshot[];
  /** Reloads the list; a failure keeps the previous one (the feature is a convenience). */
  refresh(): Promise<void>;
}

export const useAutoSnapshots = create<AutoSnapshotsState>()((set) => ({
  list: [],
  refresh: async () => {
    try {
      set({ list: await listAutoSnapshots() });
    } catch {
      /* Keep what is shown. */
    }
  },
}));

/** Loads the list now and again whenever the back end says it changed; returns the cleanup. */
export function watchAutoSnapshots(): () => void {
  const refresh = () => void useAutoSnapshots.getState().refresh();
  refresh();
  const subscription = onSnapshotsChanged(refresh).catch(() => () => {});
  return () => {
    void subscription.then((unlisten) => unlisten());
  };
}
