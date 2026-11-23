/** Flows behind the Snapshots menu: IPC calls, the store and the feedback shown to the user. */
import {
  clearBaseline,
  compareWithAutoSnapshot,
  compareWithSnapshot,
  deleteAutoSnapshot,
  openSnapshot,
  saveSnapshot,
} from './api';
import { useAutoSnapshots } from './autoSnapshots';
import type { MenuAction } from './contextMenu';
import { useFileActions } from './fileActions';
import { runScan } from './scanController';
import { canOpenSnapshot } from './snapshotsModel';
import { useAppStore } from './store';
import { strings } from './strings';

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

function fail(title: string, error: unknown): void {
  useAppStore.setState({ error: message(error), errorTitle: title });
}

/** Saves the shown result as a snapshot file; the user picks the location in a dialog. */
export async function saveCurrentSnapshot(): Promise<void> {
  const { resultScanId, scanning } = useAppStore.getState();
  if (resultScanId === null || scanning) return;
  try {
    const path = await saveSnapshot(resultScanId);
    if (path) useFileActions.getState().setNotice(strings.snapshotSaved(path));
  } catch (error) {
    fail(strings.snapshotSaveFailed, error);
  }
}

/** Asks for a snapshot file and starts comparing the shown result with it. */
export async function compareWithFile(): Promise<void> {
  const { resultScanId, scanning } = useAppStore.getState();
  if (resultScanId === null || scanning) return;
  try {
    const info = await compareWithSnapshot(resultScanId);
    // A newer result replaced the one that was compared; the back end already dropped it.
    if (info && useAppStore.getState().resultScanId === resultScanId) {
      useAppStore.getState().setBaseline(info);
    }
  } catch (error) {
    fail(strings.snapshotCompareFailed, error);
  }
}

/** Opens a saved automatic snapshot as the shown (read-only) result. */
export async function openAutoSnapshot(id: string): Promise<void> {
  if (!canOpenSnapshot(useAppStore.getState())) return;
  try {
    const opened = await openSnapshot(id);
    // A scan started meanwhile will replace the result anyway; do not fight it.
    if (!useAppStore.getState().scanning) useAppStore.getState().handleOpened(opened);
  } catch (error) {
    fail(strings.snapshotOpenFailed, error);
  }
}

/** Compares the shown live result with a saved automatic snapshot. */
export async function compareWithAutoSnapshotId(id: string): Promise<void> {
  const { resultScanId, scanning } = useAppStore.getState();
  if (resultScanId === null || scanning) return;
  try {
    const info = await compareWithAutoSnapshot(resultScanId, id);
    if (useAppStore.getState().resultScanId === resultScanId) {
      useAppStore.getState().setBaseline(info);
    }
  } catch (error) {
    fail(strings.snapshotCompareFailed, error);
  }
}

/** Deletes a saved automatic snapshot; the list refreshes through the back end's event. */
export async function removeAutoSnapshot(id: string): Promise<void> {
  try {
    await deleteAutoSnapshot(id);
  } catch (error) {
    fail(strings.snapshotDeleteFailed, error);
  }
  void useAutoSnapshots.getState().refresh();
}

/** Scans again the location a snapshot view shows. */
export function rescanSnapshotTarget(): void {
  const { resultTarget, scanning } = useAppStore.getState();
  if (resultTarget !== null && !scanning) void runScan(resultTarget);
}

/**
 * Leaves the comparison view. The back end drops the baseline first, so the views that
 * refetch their rows afterwards no longer receive change data.
 */
export function stopComparing(): void {
  clearBaseline().then(
    () => useAppStore.getState().setBaseline(null),
    (error: unknown) => fail(strings.snapshotCompareFailed, error),
  );
}

/** Runs a Snapshots menu entry. Actions of other menus are ignored. */
export function runSnapshotAction(action: MenuAction): Promise<void> {
  switch (action) {
    case 'snapshotSave':
      return saveCurrentSnapshot();
    case 'snapshotCompare':
      return compareWithFile();
    case 'snapshotStop':
      stopComparing();
      return Promise.resolve();
    default:
      return Promise.resolve();
  }
}
