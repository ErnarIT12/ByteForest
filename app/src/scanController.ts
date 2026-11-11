/** Glue between the IPC layer and the store: starting, cancelling and listening to scans. */
import {
  cancelScan,
  isElevated,
  listDrives,
  onScanDone,
  onScanError,
  onScanProgress,
  startScan,
} from './api';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useAppStore } from './store';

/** Counts `startScan` calls so that only the latest one's id becomes current. */
let requestSeq = 0;

/** Starts scanning drive `letter` ("C"). */
export async function runScan(letter: string): Promise<void> {
  const store = useAppStore.getState();
  const request = ++requestSeq;
  store.beginScan(letter);
  try {
    const id = await startScan(`${letter}:`);
    if (request === requestSeq) useAppStore.getState().scanResolved(id);
    else useAppStore.getState().noteScanId(id);
  } catch (e) {
    if (request === requestSeq) useAppStore.getState().scanFailedToStart(String(e));
  }
}

/** Asks the back end to stop the running scan; the outcome arrives as a "cancelled" error. */
export function stopScan(): void {
  cancelScan().catch((e) => useAppStore.setState({ error: String(e) }));
}

/** Reloads the drive list (sizes and free space). */
function refreshDrives(): void {
  const store = useAppStore.getState();
  listDrives().then(store.setDrives, (e) => store.setDrivesError(String(e)));
}

/** Loads the drive list and the elevation flag. */
export function loadEnvironment(): void {
  const store = useAppStore.getState();
  refreshDrives();
  isElevated().then(store.setElevated, () => store.setElevated(false));
}

/** Subscribes to the scan events; the returned function unsubscribes. */
export function listenToScanEvents(): () => void {
  const store = useAppStore.getState();
  const pending: Promise<UnlistenFn>[] = [
    onScanProgress(store.handleProgress),
    onScanDone((d) => {
      store.handleDone(d);
      // Free space may have changed since start-up; keep "used of total" current.
      refreshDrives();
    }),
    onScanError(store.handleError),
  ];
  return () => {
    for (const p of pending) p.then((unlisten) => unlisten()).catch(() => {});
  };
}
