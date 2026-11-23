/** Flows behind the Snapshots menu: IPC calls, the store and the feedback shown to the user. */
import { clearBaseline, compareWithSnapshot, saveSnapshot } from './api';
import type { MenuAction } from './contextMenu';
import { useFileActions } from './fileActions';
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

/** Leaves the comparison view. The views switch back at once; the back end follows. */
export function stopComparing(): void {
  useAppStore.getState().setBaseline(null);
  clearBaseline().catch((error: unknown) => fail(strings.snapshotCompareFailed, error));
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
