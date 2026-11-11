import { formatBytes, formatInt, formatSeconds } from '../format';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Scan summary, drive usage and the elevation badge. */
export function StatusBar() {
  const stats = useAppStore((s) => s.stats);
  const elevated = useAppStore((s) => s.elevated);
  const drives = useAppStore((s) => s.drives);
  const resultTarget = useAppStore((s) => s.resultTarget);
  const drive = drives?.find((d) => d.letter === resultTarget);

  const fallback = stats?.fallback_reason ?? null;
  const issues = stats ? stats.errors + stats.skipped_records : 0;
  const used = drive ? drive.total - drive.free : 0;

  return (
    <footer className="statusbar">
      <span className="status-summary">
        {stats
          ? strings.statusSummary(
              formatInt(stats.files),
              formatInt(stats.dirs),
              formatSeconds(stats.elapsed_ms),
              strings.methodName[stats.method],
            )
          : strings.ready}
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
      {elevated !== null && (
        <span
          className={`badge${elevated ? ' admin' : ''}`}
          title={fallback ?? (elevated ? strings.adminHint : strings.standardHint)}
        >
          {elevated ? (
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <path d="M8 1.5l5.5 2v4c0 3.4-2.3 5.9-5.5 7-3.2-1.1-5.5-3.6-5.5-7v-4z" />
            </svg>
          ) : null}
          {elevated ? strings.admin : strings.standard}
        </span>
      )}
    </footer>
  );
}
