import { RefreshCw } from 'lucide-react';
import { Icon } from './Icon';
import { runScan, stopScan } from '../scanController';
import { scanTargetFor } from '../scanTargets';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Primary "Scan" button, or "Cancel" while a scan runs. */
export function ScanButton({ compact = false }: { compact?: boolean }) {
  const scanning = useAppStore((s) => s.scanning);
  const target = useAppStore((s) => s.target);
  const shownTarget = useAppStore((s) => s.resultTarget);
  const scanTarget = target === null ? null : scanTargetFor(target);

  if (scanning) {
    return (
      <button type="button" className="btn" aria-label={strings.cancelAria} onClick={stopScan}>
        {strings.cancel}
      </button>
    );
  }
  const refresh = scanTarget !== null && scanTarget === shownTarget;
  const iconOnly = compact && refresh;
  return (
    <button
      type="button"
      className={iconOnly ? 'icon-btn refresh-scan' : 'btn primary'}
      aria-label={refresh ? strings.rescan : strings.scanAria}
      title={iconOnly ? strings.rescan : undefined}
      disabled={!scanTarget}
      onClick={() => scanTarget && void runScan(scanTarget)}
    >
      {iconOnly ? <Icon icon={RefreshCw} /> : refresh ? strings.rescan : strings.scan}
    </button>
  );
}
