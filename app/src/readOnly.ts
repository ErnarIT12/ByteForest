/** Guard for actions that change the disk: a snapshot view is read-only. */
import { useFileActions } from './fileActions';
import { useAppStore } from './store';
import { strings } from './strings';
import { canChangeDisk } from './snapshotsModel';

/** True when the shown result may be changed; otherwise tells the user why not and returns false. */
export function guardWritable(): boolean {
  if (canChangeDisk(useAppStore.getState().resultSource)) return true;
  useFileActions.getState().setNotice(strings.snapshotReadOnly);
  return false;
}
