/** Glue between the IPC layer and the store: starting, cancelling and listening to scans. */
import {
  cancelScan,
  isElevated,
  listDrives,
  restartElevated,
  takeStartupScan,
  onScanDone,
  onScanError,
  onScanProgress,
  startScan,
} from './api';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { driveLetterOf, isDeclined } from './elevation';
import { useAppStore } from './store';

/** Counts `startScan` calls so that only the latest one's id becomes current. */
let requestSeq = 0;

/** Starts scanning drive `letter` ("C"), or the folder `path` when given. */
export async function runScan(letter: string, path?: string): Promise<void> {
  const store = useAppStore.getState();
  const request = ++requestSeq;
  store.beginScan(letter);
  try {
    const id = await startScan(path ?? `${letter}:`);
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
function refreshDrives(): Promise<void> {
  const store = useAppStore.getState();
  return listDrives().then(store.setDrives, (e) => store.setDrivesError(String(e)));
}

/**
 * Restarts the app as administrator, passing `target` (a drive letter) to scan afterwards.
 * Refusing the UAC prompt is not an error; on success the process exits.
 */
export async function requestElevation(target: string): Promise<void> {
  const store = useAppStore.getState();
  store.setElevating(true);
  try {
    await restartElevated(`${target}:`);
    useAppStore.getState().setElevating(false);
  } catch (e) {
    if (isDeclined(e)) useAppStore.getState().setElevating(false);
    else useAppStore.getState().elevationFailed(String(e));
  }
}

/** Starts the scan requested with `--scan` after an elevated restart, if any. */
async function runStartupScan(): Promise<void> {
  let requested: string | null;
  try {
    requested = await takeStartupScan();
  } catch {
    return;
  }
  if (!requested) return;
  const letter = driveLetterOf(requested);
  const store = useAppStore.getState();
  if (letter !== null) {
    if (store.drives?.some((d) => d.letter.toUpperCase() === letter)) store.setTarget(letter);
    await runScan(letter);
  } else {
    await runScan(requested.slice(0, 1).toUpperCase(), requested);
  }
}

/** Loads the drive list and the elevation flag. */
export function loadEnvironment(): void {
  const store = useAppStore.getState();
  void refreshDrives().then(runStartupScan);
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
