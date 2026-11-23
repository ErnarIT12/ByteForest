import { rescanSnapshotTarget } from '../snapshots';
import { formatUnixMsFull } from '../snapshotsModel';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Says the shown result is a saved snapshot (read-only) and offers a live rescan. */
export function SnapshotBanner() {
  const source = useAppStore((s) => s.resultSource);
  const target = useAppStore((s) => s.resultTarget);
  const scanning = useAppStore((s) => s.scanning);
  if (source.kind !== 'snapshot' || target === null) return null;
  const text = strings.snapshotBanner(target, formatUnixMsFull(source.created_unix_ms));
  return (
    <div className="compare-banner snapshot-banner" role="group" aria-label={strings.snapshotBannerLabel}>
      <span className="compare-text" title={text}>{text}</span>
      <button type="button" className="btn primary" disabled={scanning} onClick={rescanSnapshotTarget}>
        {strings.snapshotRescan}
      </button>
    </div>
  );
}
