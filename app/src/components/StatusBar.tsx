import { formatBytes, formatInt, formatSeconds } from '../format';
import { canOfferFastScan, driveLetterOf } from '../elevation';
import { requestElevation } from '../scanController';
import { statusBadge } from '../statusBadge';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { Icon, ShieldCheck } from './Icon';

/** Scan summary, drive usage and the elevation badge. */
export function StatusBar() {
  const stats = useAppStore((s) => s.stats);
  const excluded = useAppStore((s) => s.excluded);
  const elevated = useAppStore((s) => s.elevated);
  const isMac = useAppStore((s) => s.isMac);
  const drives = useAppStore((s) => s.drives);
  const resultTarget = useAppStore((s) => s.resultTarget);
  const target = useAppStore((s) => s.target);
  const elevating = useAppStore((s) => s.elevating);
  const scanning = useAppStore((s) => s.scanning);
  const isSnapshot = useAppStore((s) => s.resultSource.kind === 'snapshot');
  const resultLetter = resultTarget === null ? null : driveLetterOf(resultTarget);
  const drive = drives?.find((d) =>
    isMac ? d.path === resultTarget : d.letter?.toUpperCase() === resultLetter,
  );

  const issues = stats ? stats.errors + stats.skipped_records : 0;
  const used = drive ? Math.max(0, drive.total - drive.free) : 0;
  const badge = elevated === null || isMac || isSnapshot ? null : statusBadge(elevated, stats);

  const clickable =
    elevated === false && target !== null && canOfferFastScan(false, target, drives ?? []);

  return (
    <footer className="statusbar">
      <span className="status-summary">
        {stats && isSnapshot
          ? strings.statusSnapshot(formatInt(stats.files), formatInt(stats.dirs))
          : stats
          ? strings.statusSummary(
              formatInt(stats.files),
              formatInt(stats.dirs),
              formatSeconds(stats.elapsed_ms),
              strings.methodName[stats.method],
            )
          : strings.ready}
        {excluded && (
          <span className="status-excluded" title={strings.excludedHint}>
            {' · '}
            {strings.excludedSummary(excluded.items, formatInt(excluded.items), formatBytes(excluded.bytes))}
          </span>
        )}
        {issues > 0 && (
          <span className="status-issues" title={strings.scanIssuesHint}>
            {' · '}
            {strings.scanIssues(formatInt(issues))}
          </span>
        )}
      </span>
      {drive && drive.total > 0 && (
        <span className="status-usage">
          <span className="usage-bar" aria-hidden="true">
            <span className="usage-fill" style={{ width: `${(100 * used) / drive.total}%` }} />
          </span>
          {strings.used(formatBytes(used), formatBytes(drive.total))}
        </span>
      )}
      <span className="status-spacer" />
      {badge &&
        (clickable ? (
          <button
            type="button"
            className="badge badge-action"
            title={strings.badgeRestartHint}
            disabled={elevating || scanning}
            onClick={() => void requestElevation(`${target}:`)}
          >
            {badge.label}
          </button>
        ) : (
          <span className={`badge${badge.admin ? ' admin' : ''}`} title={badge.title}>
            {badge.admin ? <Icon icon={ShieldCheck} size={12} /> : null}
            {badge.label}
          </span>
        ))}
    </footer>
  );
}
