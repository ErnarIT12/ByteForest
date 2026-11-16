import { canOfferFastScan } from '../elevation';
import { requestElevation } from '../scanController';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** "Fast scan (admin)": restarts elevated and scans the selected NTFS drive. */
export function FastScanButton() {
  const elevated = useAppStore((s) => s.elevated);
  const target = useAppStore((s) => s.target);
  const drives = useAppStore((s) => s.drives);
  const scanning = useAppStore((s) => s.scanning);
  const elevating = useAppStore((s) => s.elevating);

  if (elevated !== false || target === null || drives === null) return null;
  if (!canOfferFastScan(elevated, target, drives)) return null;
  return (
    <button
      type="button"
      className="btn"
      aria-label={strings.fastScanAria}
      disabled={scanning || elevating}
      onClick={() => void requestElevation(`${target}:`)}
    >
      {elevating ? strings.waitingForPermission : strings.fastScan}
    </button>
  );
}
