import { shouldShowElevationBar } from '../elevation';
import { formatSeconds } from '../format';
import { requestElevation } from '../scanController';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { Icon, X } from './Icon';

/** Hint under the top bar after a slow scan: offers a restart with admin rights. */
export function ElevationBar() {
  const elevated = useAppStore((s) => s.elevated);
  const stats = useAppStore((s) => s.stats);
  const target = useAppStore((s) => s.resultTarget);
  const drives = useAppStore((s) => s.drives);
  const dismissed = useAppStore((s) => s.elevationBarDismissed);
  const elevating = useAppStore((s) => s.elevating);
  const scanning = useAppStore((s) => s.scanning);
  const dismiss = useAppStore((s) => s.dismissElevationBar);

  if (scanning) return null;
  if (elevated === null || target === null || drives === null || stats === null) return null;
  if (!shouldShowElevationBar(elevated, stats, target, drives, dismissed)) return null;
  return (
    <div className="elevation-bar" role="status">
      <span>{strings.elevationBarText(formatSeconds(stats.elapsed_ms))}</span>
      <button
        type="button"
        className="btn"
        disabled={elevating || scanning}
        onClick={() => void requestElevation(target)}
      >
        {elevating ? strings.waitingForPermission : strings.restartAsAdmin}
      </button>
      <button type="button" className="icon-btn" aria-label={strings.dismiss} onClick={dismiss}>
        <Icon icon={X} />
      </button>
    </div>
  );
}
