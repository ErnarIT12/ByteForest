import { runScan, stopScan } from '../scanController';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Primary "Scan" button, or "Cancel" while a scan runs. */
export function ScanButton() {
  const scanning = useAppStore((s) => s.scanning);
  const target = useAppStore((s) => s.target);
  const shownTarget = useAppStore((s) => s.resultTarget);

  if (scanning) {
    return (
      <button type="button" className="btn" aria-label={strings.cancelAria} onClick={stopScan}>
        {strings.cancel}
      </button>
    );
  }
  return (
    <button
      type="button"
      className="btn primary"
      aria-label={strings.scanAria}
      disabled={!target}
      onClick={() => target && void runScan(target)}
    >
      {target !== null && target === shownTarget ? strings.rescan : strings.scan}
    </button>
  );
}
