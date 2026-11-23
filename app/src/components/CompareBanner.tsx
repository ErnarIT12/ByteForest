import { formatDelta, formatUnixMs } from '../format';
import { stopComparing } from '../snapshots';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Says which snapshot the result is compared with and offers to stop. */
export function CompareBanner() {
  const baseline = useAppStore((s) => s.baseline);
  if (!baseline) return null;
  const text = strings.comparingWith(
    baseline.label,
    formatUnixMs(baseline.created_unix_ms),
    formatDelta(baseline.total_delta),
  );
  return (
    <div className="compare-banner" role="group" aria-label={strings.compareBannerLabel}>
      <span className="compare-text" title={text}>{text}</span>
      <button type="button" className="btn" onClick={stopComparing}>{strings.stopComparing}</button>
    </div>
  );
}
