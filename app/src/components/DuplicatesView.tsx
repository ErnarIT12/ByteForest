import { useEffect, useMemo, useRef, useState } from 'react';
import {
  SCAN_CANCELLED,
  cancelDuplicates,
  duplicateCandidates,
  getCachedDuplicates,
  onDupesDone,
  onDupesError,
  onDupesProgress,
  startDuplicates,
  type DuplicateCandidatesInfo,
  type DuplicateFilter,
  type DuplicateGroup,
  type DuplicateResult,
  type FileCategory,
  type HashProgress,
  type NodeView,
} from '../api';
import { toggleQueued, useDiscardPile } from '../discardPile';
import { allCopiesQueued, planAutoSelect, type KeepRule } from '../dupesModel';
import { formatBytes, formatFiletime, formatInt } from '../format';
import { useColorScheme } from '../hooks/useColorScheme';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { CATEGORIES, categoryColor } from '../treemapPaint';
import { FileKindIcon } from './FileKindIcon';
import { CircleAlert, Icon, Search, X } from './Icon';

const DEFAULT_MIN_SIZE = 1024 * 1024;

const MIN_SIZES = [
  { label: '100 KB+', value: 100 * 1024 },
  { label: '1 MB+', value: 1024 * 1024 },
  { label: '10 MB+', value: 10 * 1024 * 1024 },
  { label: '100 MB+', value: 100 * 1024 * 1024 },
  { label: '1 GB+', value: 1024 * 1024 * 1024 },
  { label: 'Any (> 0 B)', value: 1 },
] as const;

interface CandidateState {
  key: string;
  generation: number;
  info?: DuplicateCandidatesInfo;
  error?: string;
}

interface ResultState {
  key: string;
  generation: number;
  result: DuplicateResult;
}

interface JobGate {
  pending: boolean;
  jobId: number | null;
  lastKnown: number;
  lastFinished: number;
  filterKey: string;
}

function acceptJob(gate: JobGate, jobId: number): boolean {
  if (gate.pending) return jobId > gate.lastKnown;
  return gate.jobId !== null && jobId === gate.jobId;
}

function progressPercent(progress: HashProgress | null): number {
  if (!progress) return 0;
  if (progress.bytes_total > 0) {
    return Math.min(100, Math.round((progress.bytes_done * 100) / progress.bytes_total));
  }
  if (progress.files_total > 0) {
    return Math.min(100, Math.round((progress.files_done * 100) / progress.files_total));
  }
  return 0;
}

export function DuplicatesView({ scanId, root, onOpen, opening, navigationError }: {
  scanId: number;
  root: NodeView;
  onOpen: (id: number) => void;
  opening: boolean;
  navigationError: string | null;
}) {
  const theme = useColorScheme();
  const isLive = useAppStore((s) => s.resultSource.kind === 'live');
  const scanning = useAppStore((s) => s.scanning);
  const generation = useAppStore((s) => s.generation);
  const preset = useAppStore((s) => s.dupesPreset);
  const queued = useDiscardPile((s) => s.idSet);
  const busy = useDiscardPile((s) => s.busy);
  const writable = !scanning && isLive;

  const [name, setName] = useState('');
  const [minSize, setMinSize] = useState(DEFAULT_MIN_SIZE);
  const [category, setCategory] = useState<FileCategory | ''>('');
  const [includeHidden, setIncludeHidden] = useState(false);
  const [scope, setScope] = useState<{ id: number; name: string } | null>(null);

  const [seenScanId, setSeenScanId] = useState(scanId);
  const [appliedSeq, setAppliedSeq] = useState(0);
  const [candidateState, setCandidateState] = useState<CandidateState | null>(null);
  const [resultState, setResultState] = useState<ResultState | null>(null);
  const [running, setRunning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [progress, setProgress] = useState<HashProgress | null>(null);
  const [runError, setRunError] = useState<string | null>(null);

  // Reset local state when the shown scan changes.
  if (seenScanId !== scanId) {
    setSeenScanId(scanId);
    setScope(null);
    setAppliedSeq(0);
    setCandidateState(null);
    setResultState(null);
    setRunning(false);
    setCancelling(false);
    setProgress(null);
    setRunError(null);
  }

  // Apply a folder-scoped preset opened from context menu or node details.
  if (preset && preset.seq !== appliedSeq) {
    setAppliedSeq(preset.seq);
    setScope(preset.under.id === root.id ? null : preset.under);
  } else if (!preset && appliedSeq !== 0) {
    setAppliedSeq(0);
  }

  const filter = useMemo<DuplicateFilter>(
    () => ({
      min_size: minSize,
      under: scope?.id,
      category: category || undefined,
      name,
      include_hidden: includeHidden,
    }),
    [minSize, scope, category, name, includeHidden],
  );

  const filterKey = `${scanId}:${filter.min_size}:${filter.under ?? ''}:${filter.category ?? ''}:${filter.name}:${filter.include_hidden ? 1 : 0}`;
  const currentCandidates =
    candidateState?.key === filterKey && candidateState.generation === generation
      ? candidateState
      : null;
  const currentResult =
    resultState?.key === filterKey && resultState.generation === generation
      ? resultState.result
      : null;

  const jobGateRef = useRef<JobGate>({
    pending: false,
    jobId: null,
    lastKnown: 0,
    lastFinished: 0,
    filterKey: '',
  });

  // Subscribe to backend duplicate hashing events for the active scan.
  useEffect(() => {
    let alive = true;
    const unlisteners: (() => void)[] = [];
    const register = async () => {
      const fns = await Promise.all([
        onDupesProgress((e) => {
          if (!alive || e.scan_id !== scanId) return;
          if (!acceptJob(jobGateRef.current, e.job_id)) return;
          setProgress(e.progress);
        }),
        onDupesDone((e) => {
          if (!alive || e.scan_id !== scanId) return;
          const gate = jobGateRef.current;
          if (!acceptJob(gate, e.job_id)) return;
          gate.lastKnown = Math.max(gate.lastKnown, e.job_id);
          gate.lastFinished = e.job_id;
          gate.pending = false;
          const gen = useAppStore.getState().generation;
          setRunning(false);
          setCancelling(false);
          setProgress(null);
          setRunError(null);
          setResultState({ key: gate.filterKey, generation: gen, result: e.result });
          setCandidateState((prev) =>
            prev && prev.key === gate.filterKey && prev.info
              ? { ...prev, generation: gen, info: { ...prev.info, cached: true } }
              : prev,
          );
        }),
        onDupesError((e) => {
          if (!alive || e.scan_id !== scanId) return;
          const gate = jobGateRef.current;
          if (!acceptJob(gate, e.job_id)) return;
          gate.lastKnown = Math.max(gate.lastKnown, e.job_id);
          gate.lastFinished = e.job_id;
          gate.pending = false;
          setRunning(false);
          setCancelling(false);
          setProgress(null);
          if (e.message !== SCAN_CANCELLED) {
            setRunError(e.message);
          }
        }),
      ]);
      if (!alive) {
        for (const fn of fns) fn();
      } else {
        unlisteners.push(...fns);
      }
    };
    void register();
    return () => {
      alive = false;
      for (const fn of unlisteners) fn();
    };
  }, [scanId]);

  // Estimate candidates and load any matching cached result (also refreshes on generation bump
  // after files are moved to the trash so deleted copies disappear without re-hashing).
  useEffect(() => {
    if (!isLive) return;
    let alive = true;
    const delay = name.length > 0 ? 180 : 0;
    const timer = window.setTimeout(() => {
      void Promise.all([
        duplicateCandidates(scanId, filter),
        getCachedDuplicates(scanId, filter),
      ]).then(
        ([info, cached]) => {
          if (!alive) return;
          setCandidateState({ key: filterKey, generation, info });
          if (cached !== null) {
            setResultState({ key: filterKey, generation, result: cached });
          }
        },
        (error: unknown) => {
          if (!alive) return;
          setCandidateState({ key: filterKey, generation, error: String(error) });
        },
      );
    }, delay);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [isLive, scanId, filter, filterKey, generation, name]);

  const stopActiveJob = () => {
    if (!running) return;
    jobGateRef.current = {
      ...jobGateRef.current,
      pending: false,
      jobId: null,
    };
    setRunning(false);
    setCancelling(false);
    setProgress(null);
    void cancelDuplicates().catch(() => {});
  };

  const handleStart = async () => {
    if (!isLive) return;
    setRunning(true);
    setCancelling(false);
    setProgress(null);
    setRunError(null);
    jobGateRef.current = {
      pending: true,
      jobId: null,
      lastKnown: jobGateRef.current.lastKnown,
      lastFinished: jobGateRef.current.lastFinished,
      filterKey,
    };
    try {
      const jobId = await startDuplicates(scanId, filter);
      const gate = jobGateRef.current;
      // Ignore if superseded by another start or filter change while awaiting invoke.
      if (gate.filterKey !== filterKey) return;
      gate.lastKnown = Math.max(gate.lastKnown, jobId);
      gate.jobId = jobId;
      gate.pending = false;
      if (gate.lastFinished === jobId) {
        setRunning(false);
      }
    } catch (error) {
      jobGateRef.current.pending = false;
      setRunning(false);
      setCancelling(false);
      setRunError(String(error));
    }
  };

  const handleCancel = () => {
    if (!running || cancelling) return;
    setCancelling(true);
    void cancelDuplicates().catch(() => {});
  };

  const applyKeepRule = (rule: KeepRule, groups: readonly DuplicateGroup[]) => {
    if (!writable || busy) return;
    const { keepIds, queueIds } = planAutoSelect(groups, rule);
    useDiscardPile.getState().updateBatch(queueIds, keepIds);
  };

  if (!isLive) {
    return (
      <main className="explorer dupes-view">
        <header className="scan-heading">
          <h1>{strings.dupesTitle}</h1>
        </header>
        <section className="dupes-panel dupes-readonly" aria-label={strings.dupesAria}>
          <p>{strings.dupesSnapshotNotice}</p>
        </section>
      </main>
    );
  }

  const candidateInfo = currentCandidates?.info;
  const pct = progressPercent(progress);
  const canStart = !running && (candidateInfo === undefined || candidateInfo.files > 0);
  const activeError = navigationError || runError || currentCandidates?.error;

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
        {scope && (
          <div className="search-scope" role="group" aria-label={strings.dupesScopeLabel}>
            <span title={scope.name}>{strings.dupesInFolder(scope.name)}</span>
            <button type="button" className="btn" onClick={() => { stopActiveJob(); setScope(null); }}>
              {strings.dupesSearchEntireScan}
            </button>
          </div>
        )}

        <div className="dupes-filters">
          <div className="swift-search">
            <Icon icon={Search} size={15} className="swift-search-icon" />
            <input
              type="search"
              value={name}
              maxLength={255}
              aria-label={strings.dupesFileName}
              placeholder={strings.dupesFileNamePlaceholder}
              onChange={(event) => {
                stopActiveJob();
                setName(event.target.value);
              }}
            />
            {name.length > 0 && (
              <button
                type="button"
                className="swift-clear"
                aria-label="Clear search"
                onClick={() => {
                  stopActiveJob();
                  setName('');
                }}
              >
                <Icon icon={X} size={12} />
              </button>
            )}
          </div>

          <select
            className="swift-select"
            aria-label={strings.dupesMinSize}
            value={minSize}
            onChange={(event) => {
              stopActiveJob();
              setMinSize(Number(event.target.value));
            }}
          >
            {MIN_SIZES.map(({ label, value }) => (
              <option key={value} value={value}>{label}</option>
            ))}
          </select>

          <label className="dupes-checkbox">
            <input
              type="checkbox"
              checked={includeHidden}
              onChange={(event) => {
                stopActiveJob();
                setIncludeHidden(event.target.checked);
              }}
            />
            <span>{strings.dupesIncludeHidden}</span>
          </label>
        </div>

        <div className="category-strip" role="group" aria-label={strings.searchCategory}>
          <button
            type="button"
            className="chip"
            aria-pressed={!category}
            onClick={() => {
              stopActiveJob();
              setCategory('');
            }}
          >
            {strings.searchAnyType}
          </button>
          {CATEGORIES.map((type) => (
            <button
              key={type}
              type="button"
              className="chip"
              aria-pressed={category === type}
              onClick={() => {
                stopActiveJob();
                setCategory(category === type ? '' : type);
              }}
            >
              <span
                className="category-dot"
                aria-hidden="true"
                style={{ background: categoryColor(theme, type, 'file') }}
              />
              {strings.categoryNames[type]}
            </button>
          ))}
        </div>

        <div className="dupes-action-bar">
          <div className="dupes-estimate" role="status">
            {opening
              ? 'Locating file…'
              : !currentCandidates
                ? strings.dupesEstimating
                : candidateInfo && (
                    candidateInfo.files === 0
                      ? strings.dupesCandidatesNone
                      : strings.dupesCandidatesEstimate(
                          formatInt(candidateInfo.files),
                          formatInt(candidateInfo.groups),
                          formatBytes(candidateInfo.bytes),
                        )
                  )}
          </div>
          <div className="dupes-action-buttons">
            {running ? (
              <button
                type="button"
                className="btn"
                disabled={cancelling}
                onClick={handleCancel}
              >
                {cancelling ? strings.dupesCancelling : strings.dupesCancel}
              </button>
            ) : (
              <button
                type="button"
                className="btn primary"
                disabled={!canStart}
                onClick={() => void handleStart()}
              >
                {currentResult ? strings.dupesRecheck : strings.dupesFind}
              </button>
            )}
          </div>
        </div>

        {running && (
          <div className="dupes-progress" role="status" aria-live="polite">
            <div className="dupes-progress-text">
              {progress
                ? strings.dupesProgressDetail(
                    strings.dupesPhase[progress.phase],
                    formatInt(progress.files_done),
                    formatInt(progress.files_total),
                    formatBytes(progress.bytes_done),
                    formatBytes(progress.bytes_total),
                  )
                : strings.dupesPhase.Partial}
            </div>
            <div
              className="dupes-progress-track"
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={pct}
            >
              <span className="dupes-progress-fill" style={{ width: `${pct}%` }} />
            </div>
          </div>
        )}

        {activeError && (
          <p className="dupes-error" role="alert">{activeError}</p>
        )}

        {currentResult && (
          <div className="dupes-results-header">
            <div className="dupes-results-summary">
              {currentResult.total_groups === 0 ? (
                <strong>{strings.dupesNoneFound}</strong>
              ) : (
                <strong>
                  {strings.dupesResultSummary(
                    formatInt(currentResult.total_groups),
                    currentResult.total_groups === 1,
                    formatInt(currentResult.total_files),
                    formatBytes(currentResult.reclaimable_bytes),
                  )}
                </strong>
              )}
              {currentResult.total_groups > currentResult.groups.length && (
                <span className="dupes-meta-note">
                  {strings.dupesResultCapped(
                    formatInt(currentResult.groups.length),
                    formatInt(currentResult.total_groups),
                  )}
                </span>
              )}
              {currentResult.skipped > 0 && (
                <span className="dupes-meta-note">
                  {strings.dupesSkippedNotice(
                    formatInt(currentResult.skipped),
                    currentResult.skipped === 1,
                  )}
                </span>
              )}
            </div>

            {currentResult.groups.length > 0 && writable && (
              <div
                className="dupes-autoselect"
                role="group"
                aria-label={strings.dupesAutoSelectLabel}
              >
                <button
                  type="button"
                  className="btn"
                  disabled={busy}
                  onClick={() => applyKeepRule('oldest', currentResult.groups)}
                >
                  {strings.dupesKeepOldest}
                </button>
                <button
                  type="button"
                  className="btn"
                  disabled={busy}
                  onClick={() => applyKeepRule('newest', currentResult.groups)}
                >
                  {strings.dupesKeepNewest}
                </button>
                <button
                  type="button"
                  className="btn"
                  disabled={busy}
                  onClick={() => applyKeepRule('shortestPath', currentResult.groups)}
                >
                  {strings.dupesKeepShortestPath}
                </button>
              </div>
            )}
          </div>
        )}

        <div className="dupes-groups-scroll">
          {currentResult && currentResult.groups.length > 0 && (
            <ul className="dupes-groups">
              {currentResult.groups.map((group) => {
                const allQueued = allCopiesQueued(group, queued);
                return (
                  <li key={`${group.size}:${group.hash}`} className="dupe-group">
                    <div className="dupe-group-header">
                      <div className="dupe-group-stats">
                        <strong className="dupe-group-size">{formatBytes(group.size)}</strong>
                        <span className="dupe-group-meta">
                          {strings.dupesGroupCopies(formatInt(group.copies.length))}
                        </span>
                        <span className="dupe-group-reclaimable">
                          {strings.dupesGroupReclaimable(formatBytes(group.reclaimable))}
                        </span>
                        <code className="dupe-hash" title={strings.dupesHashTitle(group.hash)}>
                          {group.hash.slice(0, 12)}
                        </code>
                        {allQueued && (
                          <span className="dupe-warning" role="status">
                            <Icon icon={CircleAlert} size={14} />
                            {strings.dupesAllCopiesQueued}
                          </span>
                        )}
                      </div>
                      {writable && (
                        <div className="dupe-group-actions" role="group" aria-label={strings.dupesAutoSelectLabel}>
                          <button
                            type="button"
                            className="dupe-mini-btn"
                            disabled={busy}
                            onClick={() => applyKeepRule('oldest', [group])}
                          >
                            {strings.dupesKeepOldest}
                          </button>
                          <button
                            type="button"
                            className="dupe-mini-btn"
                            disabled={busy}
                            onClick={() => applyKeepRule('newest', [group])}
                          >
                            {strings.dupesKeepNewest}
                          </button>
                          <button
                            type="button"
                            className="dupe-mini-btn"
                            disabled={busy}
                            onClick={() => applyKeepRule('shortestPath', [group])}
                          >
                            {strings.dupesKeepShortestPath}
                          </button>
                        </div>
                      )}
                    </div>
                    <table className="matches dupe-copies">
                      <tbody>
                        {group.copies.map((copy) => (
                          <tr key={copy.id}>
                            {writable && (
                              <td className="dupe-check-cell">
                                <input
                                  type="checkbox"
                                  checked={queued.has(copy.id)}
                                  disabled={busy}
                                  aria-label={strings.dupesSelectCopy(copy.path)}
                                  onChange={() => toggleQueued(copy.id)}
                                />
                              </td>
                            )}
                            <td className="dupe-name-cell">
                              <button
                                type="button"
                                className="match-name"
                                onClick={() => onOpen(copy.id)}
                                title={strings.dupesShowInMap(copy.name)}
                              >
                                <FileKindIcon node={copy.id} name={copy.name} directory={false} />
                                {copy.name}
                              </button>
                            </td>
                            <td className="dupe-path-cell" title={copy.path}>{copy.path}</td>
                            <td className="dupe-date-cell">{formatFiletime(copy.mtime) || '—'}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </li>
                );
              })}
            </ul>
          )}
        </div>

        {writable && queued.size > 0 && (
          <footer className="selection-bar">
            <span>{strings.dupesSelectedCount(formatInt(queued.size))}</span>
            <button
              type="button"
              className="btn primary"
              onClick={() => useAppStore.setState({ resultView: 'cleanup' })}
            >
              {strings.dupesReviewSelection}
            </button>
          </footer>
        )}
      </section>
    </main>
  );
}
