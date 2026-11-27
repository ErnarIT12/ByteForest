import type { NodeView } from '../api';
import { useDiscardPile } from '../discardPile';
import { formatBytes, formatInt } from '../format';
import { useColorScheme } from '../hooks/useColorScheme';
import { MIN_SIZES, useDuplicates } from '../hooks/useDuplicates';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { CATEGORIES, categoryColor } from '../treemapPaint';
import { DuplicateGroupCard, KEEP_RULES } from './DuplicateGroupCard';
import { Icon, Search, X } from './Icon';

function SelectionFooter() {
  const count = useDiscardPile((s) => s.ids.length);
  if (count === 0) return null;
  return (
    <footer className="selection-bar">
      <span>{strings.dupesSelectedCount(formatInt(count))}</span>
      <button
        type="button"
        className="btn primary"
        onClick={() => useAppStore.setState({ resultView: 'cleanup' })}
      >
        {strings.dupesReviewSelection}
      </button>
    </footer>
  );
}

export function DuplicatesView({ scanId, root, onOpen, opening, navigationError }: {
  scanId: number;
  root: NodeView;
  onOpen: (id: number) => void;
  opening: boolean;
  navigationError: string | null;
}) {
  const theme = useColorScheme();
  const busy = useDiscardPile((s) => s.busy);
  const d = useDuplicates(scanId, root.id);

  if (!d.isLive) {
    return (
      <main className="explorer dupes-view">
        <header className="scan-heading"><h1>{strings.dupesTitle}</h1></header>
        <section className="dupes-panel dupes-readonly" aria-label={strings.dupesAria}>
          <p>{strings.dupesSnapshotNotice}</p>
        </section>
      </main>
    );
  }

  const activeError = navigationError || d.error;
  const estimateText = opening
    ? 'Locating file…'
    : !d.currentCandidates
      ? strings.dupesEstimating
      : d.candidateInfo && (
          d.candidateInfo.files === 0
            ? strings.dupesCandidatesNone
            : strings.dupesCandidatesEstimate(
                formatInt(d.candidateInfo.files),
                formatInt(d.candidateInfo.groups),
                formatBytes(d.candidateInfo.bytes),
              )
        );

  return (
    <main className="explorer dupes-view">
      <header className="scan-heading">
        <h1>{strings.dupesTitle}</h1>
        <dl className="scan-totals">
          <div><dt>Scanned</dt><dd>{formatBytes(root.size)}</dd></div>
          <div><dt>Files</dt><dd>{formatInt(root.files)}</dd></div>
        </dl>
      </header>

      <section className="dupes-panel" aria-label={strings.dupesAria}>
        <div className="dupes-header-card">
          {d.scope && (
            <div className="search-scope" role="group" aria-label={strings.dupesScopeLabel}>
              <span title={d.scope.name}>{strings.dupesInFolder(d.scope.name)}</span>
              <button type="button" className="btn" onClick={d.clearScope}>
                {strings.dupesSearchEntireScan}
              </button>
            </div>
          )}

          <div className="dupes-filters">
            <div className="swift-search">
              <Icon icon={Search} size={15} className="swift-search-icon" />
              <input
                type="search"
                value={d.name}
                maxLength={255}
                aria-label={strings.dupesFileName}
                placeholder={strings.dupesFileNamePlaceholder}
                onChange={(event) => d.setName(event.target.value)}
              />
              {d.name.length > 0 && (
                <button type="button" className="swift-clear" aria-label="Clear search" onClick={() => d.setName('')}>
                  <Icon icon={X} size={12} />
                </button>
              )}
            </div>
            <select
              className="swift-select"
              aria-label={strings.dupesMinSize}
              value={d.minSize}
              onChange={(event) => d.setMinSize(Number(event.target.value))}
            >
              {MIN_SIZES.map(({ label, value }) => (
                <option key={value} value={value}>{label}</option>
              ))}
            </select>
            <label className="dupes-checkbox">
              <input
                type="checkbox"
                checked={d.includeHidden}
                onChange={(event) => d.setIncludeHidden(event.target.checked)}
              />
              <span>{strings.dupesIncludeHidden}</span>
            </label>
          </div>

          <div className="category-strip" role="group" aria-label={strings.searchCategory}>
            <button type="button" className="chip" aria-pressed={!d.category} onClick={() => d.setCategory('')}>
              {strings.searchAnyType}
            </button>
            {CATEGORIES.map((type) => (
              <button
                key={type}
                type="button"
                className="chip"
                aria-pressed={d.category === type}
                onClick={() => d.setCategory(d.category === type ? '' : type)}
              >
                <span className="category-dot" aria-hidden="true" style={{ background: categoryColor(theme, type, 'file') }} />
                {strings.categoryNames[type]}
              </button>
            ))}
          </div>

          <div className="dupes-action-bar">
            <div className="dupes-estimate" role="status">{estimateText}</div>
            <div className="dupes-action-buttons">
              {d.running ? (
                <button type="button" className="btn" disabled={d.cancelling} onClick={d.cancel}>
                  {d.cancelling ? strings.dupesCancelling : strings.dupesCancel}
                </button>
              ) : (
                <button type="button" className="btn primary" disabled={!d.canStart} onClick={() => void d.start()}>
                  {d.currentResult ? strings.dupesRecheck : strings.dupesFind}
                </button>
              )}
            </div>
          </div>

          {d.running && (
            <div className="dupes-progress" role="status" aria-live="polite">
              <div className="dupes-progress-text">
                {d.progress
                  ? strings.dupesProgressDetail(
                      strings.dupesPhase[d.progress.phase],
                      formatInt(d.progress.files_done),
                      formatInt(d.progress.files_total),
                      formatBytes(d.progress.bytes_done),
                      formatBytes(d.progress.bytes_total),
                    )
                  : strings.dupesPhase.Partial}
              </div>
              <div className="dupes-progress-track" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={d.progressPct}>
                <span className="dupes-progress-fill" style={{ width: `${d.progressPct}%` }} />
              </div>
            </div>
          )}

          {activeError && <p className="dupes-error" role="alert">{activeError}</p>}

          {d.currentResult && (
            <div className="dupes-results-header">
              <div className="dupes-results-summary">
                <strong>
                  {d.currentResult.total_groups === 0
                    ? strings.dupesNoneFound
                    : strings.dupesResultSummary(
                        formatInt(d.currentResult.total_groups),
                        d.currentResult.total_groups === 1,
                        formatInt(d.currentResult.total_files),
                        formatBytes(d.currentResult.reclaimable_bytes),
                      )}
                </strong>
                {d.currentResult.total_groups > d.currentResult.groups.length && (
                  <span className="dupes-meta-note">
                    {strings.dupesResultCapped(formatInt(d.currentResult.groups.length), formatInt(d.currentResult.total_groups))}
                  </span>
                )}
                {d.currentResult.skipped > 0 && (
                  <span className="dupes-meta-note">
                    {strings.dupesSkippedNotice(formatInt(d.currentResult.skipped), d.currentResult.skipped === 1)}
                  </span>
                )}
              </div>
              {d.currentResult.groups.length > 0 && d.writable && (
                <div className="dupes-autoselect" role="group" aria-label={strings.dupesAutoSelectLabel}>
                  {KEEP_RULES.map(({ rule, label }) => (
                    <button
                      key={rule}
                      type="button"
                      className="btn"
                      disabled={busy}
                      onClick={() => d.applyKeepRule(rule, d.currentResult!.groups)}
                    >
                      {label}
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        <div className="dupes-groups-scroll">
          {d.visibleGroups.length > 0 && (
            <>
              <ul className="dupes-groups">
                {d.visibleGroups.map((group) => (
                  <DuplicateGroupCard
                    key={`${group.size}:${group.hash}`}
                    group={group}
                    writable={d.writable}
                    busy={busy}
                    onKeep={d.applyKeepRule}
                    onOpen={onOpen}
                  />
                ))}
              </ul>
              {d.remainingGroups > 0 && (
                <div className="dupes-show-more">
                  <button type="button" className="btn" onClick={d.showMore}>
                    {strings.dupesShowMore(formatInt(d.nextBatchSize), formatInt(d.remainingGroups))}
                  </button>
                </div>
              )}
            </>
          )}
        </div>

        {d.writable && <SelectionFooter />}
      </section>
    </main>
  );
}
