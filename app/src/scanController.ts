/** Glue between the IPC layer and the store: starting, cancelling and listening to scans. */
import {
  cancelScan,
  pickFolder,
  isElevated,
  listDrives,
  listUserFolders,
  restartElevated,
  takeStartupScan,
  onScanDone,
  onScanError,
  onScanProgress,
  startScan,
} from './api';
import { driveLetterOf, isAcceptableStartupTarget, isDeclined } from './elevation';
import { acceptEvent, useAppStore } from './store';

import { strings } from './strings';

export async function chooseFolderAndScan(): Promise<void> {
  try {
    const folder = await pickFolder();
    if (folder) {
      useAppStore.getState().setTarget(folder);
      await runScan(folder);
    }
  } catch (error) {
    useAppStore.setState({ error: String(error), errorTitle: strings.chooseFolderFailed });
  }
}

/** Counts `startScan` calls so that only the latest one's id becomes current. */
let requestSeq = 0;

/** Starts scanning `target`: a drive root ("C:") or a folder path. It is stored as given. */
export async function runScan(target: string): Promise<void> {
  const store = useAppStore.getState();
  const request = ++requestSeq;
  store.beginScan(target);
  try {
    const id = await startScan(target);
    if (request === requestSeq) useAppStore.getState().scanResolved(id);
    else useAppStore.getState().noteScanId(id);
  } catch (e) {
    if (request === requestSeq) useAppStore.getState().scanFailedToStart(String(e));
  }
}

/** Asks the back end to stop the running scan; the outcome arrives as a "cancelled" error. */
export function stopScan(): void {
  cancelScan().catch((e) => useAppStore.setState({ error: String(e), errorTitle: null }));
}

/** Reloads the drive list (sizes and free space). */
function refreshDrives(): Promise<void> {
  const store = useAppStore.getState();
  return listDrives().then(store.setDrives, (e) => store.setDrivesError(String(e)));
}

/**
 * Restarts the app as administrator, passing `target` (a drive root such as "C:" or "C") to
 * scan afterwards. Refusing the UAC prompt is not an error. On success the process exits, so
 * `elevating` stays set to avoid the button flickering back.
 */
export async function requestElevation(target: string): Promise<void> {
  if (useAppStore.getState().elevating) return;
  useAppStore.getState().setElevating(true);
  const letter = driveLetterOf(target);
  try {
    await restartElevated(letter !== null ? `${letter}:` : target);
  } catch (e) {
    if (isDeclined(e)) useAppStore.getState().setElevating(false);
    else useAppStore.getState().elevationFailed(String(e));
  }
}

/** Resolves once the scan event listeners are registered; set by `listenToScanEvents`. */
let listenersReady: Promise<unknown> = Promise.resolve();

/** Starts the scan requested with `--scan` after an elevated restart, if any. */
async function runStartupScan(): Promise<void> {
  let requested: string | null;
  try {
    requested = await takeStartupScan();
  } catch {
    return;
  }
  if (!requested) return;
  if (!isAcceptableStartupTarget(requested)) {
    console.warn(`Ignoring --scan value that is not a drive or an absolute path: ${requested}`);
    return;
  }
  const letter = driveLetterOf(requested);
  const store = useAppStore.getState();
  if (letter !== null && store.drives?.some((d) => d.letter?.toUpperCase() === letter)) {
    store.setTarget(letter);
  } else if (letter === null) {
    store.setTarget(requested.trim());
  }
  // Events of the scan must not be missed, so wait until the listeners are registered.
  await listenersReady.catch(() => {});
  await runScan(letter !== null ? `${letter}:` : requested.trim());
}

/** Loads the drive list and the elevation flag. */
export function loadEnvironment(): void {
  const store = useAppStore.getState();
  void refreshDrives().then(runStartupScan);
  isElevated().then(store.setElevated, () => store.setElevated(false));
  // Quick locations are a convenience; without them the sidebar just lists the drives.
  listUserFolders().then(store.setUserFolders, () => {});
}

/**
 * Subscribes to the scan events. The promise resolves, once every listener settles, to an
 * idempotent cleanup function. Passing `signal` suppresses callbacks immediately on unmount
 * and unsubscribes as soon as pending registrations settle.
 */
export function listenToScanEvents(signal?: { cancelled: boolean }): Promise<() => void> {
  const store = useAppStore.getState();
  let active = true;
  const isActive = () => active && !signal?.cancelled;
  const ready = Promise.allSettled([
    onScanProgress((p) => {
      if (isActive()) store.handleProgress(p);
    }),
    onScanDone((d) => {
      if (!isActive()) return;
      const accepted = acceptEvent(useAppStore.getState().gate, d.scan_id);
      store.handleDone(d);
      // Free space may have changed since start-up; keep "used of total" current.
      if (accepted) void refreshDrives();
    }),
    onScanError((e) => {
      if (isActive()) store.handleError(e);
    }),
  ]).then((settled): (() => void) => {
    const unlisteners: (() => void)[] = [];
    for (const item of settled) {
      if (item.status === 'fulfilled') unlisteners.push(item.value);
    }
    const stop = () => {
      active = false;
      for (const unlisten of unlisteners) unlisten();
      unlisteners.length = 0;
    };
    if (signal?.cancelled) stop();
    return stop;
  });
  listenersReady = ready;
  return ready;
}
