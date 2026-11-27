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
} from '../api';
import { useDiscardPile } from '../discardPile';
import {
  DUPES_PAGE_SIZE,
  acceptDupesJob,
  beginDupesJob,
  createDupesJobGate,
  finishDupesJob,
  planAutoSelect,
  progressPercent,
  resolveDupesJob,
  stopDupesJob,
  type KeepRule,
} from '../dupesModel';
import { useAppStore } from '../store';

export const DEFAULT_MIN_SIZE = 1024 * 1024;

export const MIN_SIZES = [
  { label: '100 KB+', value: 100 * 1024 },
  { label: '1 MB+', value: 1024 * 1024 },
  { label: '10 MB+', value: 10 * 1024 * 1024 },
  { label: '100 MB+', value: 100 * 1024 * 1024 },
  { label: '1 GB+', value: 1024 * 1024 * 1024 },
  { label: 'Any (> 0 B)', value: 1 },
] as const;

export interface DupesScope {
  id: number;
  name: string;
}

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

/**
 * Filter state, candidate estimation, Tauri hashing job lifecycle, and incremental
 * group pagination for `DuplicatesView`. Mount the caller with `key={scanId}` so a new
 * scan starts from a clean state without synchronous `setState` during render.
 */
export function useDuplicates(scanId: number, rootId: number) {
  const isLive = useAppStore((s) => s.resultSource.kind === 'live');
  const writable = useAppStore((s) => !s.scanning && s.resultSource.kind === 'live');
  const generation = useAppStore((s) => s.generation);
  const preset = useAppStore((s) => s.dupesPreset);

  const [name, setNameState] = useState('');
  const [minSize, setMinSizeState] = useState(DEFAULT_MIN_SIZE);
  const [category, setCategoryState] = useState<FileCategory | ''>('');
  const [includeHidden, setIncludeHiddenState] = useState(false);
  const [scopeOverride, setScopeOverride] = useState<{ seq: number; value: DupesScope | null } | null>(null);

  const presetSeq = preset?.seq ?? 0;
  const scope: DupesScope | null =
    scopeOverride && scopeOverride.seq === presetSeq
      ? scopeOverride.value
      : preset && preset.under.id !== rootId
        ? preset.under
        : null;

  const [candidateState, setCandidateState] = useState<CandidateState | null>(null);
  const [resultState, setResultState] = useState<ResultState | null>(null);
  const [running, setRunning] = useState(false);
  const [cancelling, setCancelling] = useState(false);
  const [progress, setProgress] = useState<HashProgress | null>(null);
  const [runError, setRunError] = useState<string | null>(null);
  const [page, setPage] = useState<{ key: string; count: number }>({
    key: '',
    count: DUPES_PAGE_SIZE,
  });

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

  const jobGateRef = useRef(createDupesJobGate());
  const aliveRef = useRef(true);
  const latestFilterRef = useRef({ key: filterKey, filter });
  useEffect(() => {
    latestFilterRef.current = { key: filterKey, filter };
  }, [filterKey, filter]);

  // Subscribe to backend duplicate hashing events for the active scan.
  useEffect(() => {
    let alive = true;
    aliveRef.current = true;
    const unlisteners: (() => void)[] = [];

    const register = async () => {
      const settled = await Promise.allSettled([
        onDupesProgress((e) => {
          if (!alive || e.scan_id !== scanId) return;
          if (!acceptDupesJob(jobGateRef.current, e.job_id)) return;
          setProgress(e.progress);
        }),
        onDupesDone((e) => {
          if (!alive || e.scan_id !== scanId) return;
          const gate = jobGateRef.current;
          if (!acceptDupesJob(gate, e.job_id)) return;
          const completedKey = gate.filterKey;
          const startedGeneration = gate.startedGeneration;
          jobGateRef.current = finishDupesJob(gate, e.job_id);
          setRunning(false);
          setCancelling(false);
          setProgress(null);
          setRunError(null);
          setResultState({ key: completedKey, generation: startedGeneration, result: e.result });
          setCandidateState((prev) =>
            prev && prev.key === completedKey && prev.info
              ? { ...prev, generation: startedGeneration, info: { ...prev.info, cached: true } }
              : prev,
          );
          const latestGen = useAppStore.getState().generation;
          if (latestGen !== startedGeneration && latestFilterRef.current.key === completedKey) {
            const activeFilter = latestFilterRef.current.filter;
            void getCachedDuplicates(scanId, activeFilter)
              .then((cached) => {
                if (!alive || !cached) return;
                if (latestFilterRef.current.key !== completedKey) return;
                if (useAppStore.getState().generation !== latestGen) return;
                setResultState({ key: completedKey, generation: latestGen, result: cached });
              })
              .catch(() => {});
          }
        }),
        onDupesError((e) => {
          if (!alive || e.scan_id !== scanId) return;
          const gate = jobGateRef.current;
          if (!acceptDupesJob(gate, e.job_id)) return;
          jobGateRef.current = finishDupesJob(gate, e.job_id);
          setRunning(false);
          setCancelling(false);
          setProgress(null);
          if (e.message !== SCAN_CANCELLED) {
            setRunError(e.message);
          }
        }),
      ]);

      const registered: (() => void)[] = [];
      for (const item of settled) {
        if (item.status === 'fulfilled') registered.push(item.value);
      }
      if (!alive) {
        for (const fn of registered) fn();
      } else {
        unlisteners.push(...registered);
      }
    };
    void register();

    return () => {
      alive = false;
      aliveRef.current = false;
      for (const fn of unlisteners) fn();
      unlisteners.length = 0;
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
    if (!running && !jobGateRef.current.pending && jobGateRef.current.jobId === null) return;
    jobGateRef.current = stopDupesJob(jobGateRef.current);
    setRunning(false);
    setCancelling(false);
    setProgress(null);
    void cancelDuplicates().catch(() => {});
  };

  const setName = (value: string) => {
    stopActiveJob();
    setNameState(value);
  };

  const setMinSize = (value: number) => {
    stopActiveJob();
    setMinSizeState(value);
  };

  const setCategory = (value: FileCategory | '') => {
    stopActiveJob();
    setCategoryState(value);
  };

  const setIncludeHidden = (value: boolean) => {
    stopActiveJob();
    setIncludeHiddenState(value);
  };

  const clearScope = () => {
    stopActiveJob();
    setScopeOverride({ seq: presetSeq, value: null });
  };

  const start = async () => {
    if (!isLive) return;
    const startedGeneration = useAppStore.getState().generation;
    const nextGate = beginDupesJob(jobGateRef.current, filterKey, startedGeneration);
    const attempt = nextGate.attempt;
    jobGateRef.current = nextGate;
    setRunning(true);
    setCancelling(false);
    setProgress(null);
    setRunError(null);
    setPage({ key: filterKey, count: DUPES_PAGE_SIZE });
    try {
      const jobId = await startDuplicates(scanId, filter);
      const resolved = resolveDupesJob(jobGateRef.current, attempt, jobId);
      jobGateRef.current = resolved.gate;
      if (resolved.superseded) {
        void cancelDuplicates().catch(() => {});
        return;
      }
      if (aliveRef.current && !resolved.active) {
        setRunning(false);
      }
    } catch (error) {
      if (jobGateRef.current.attempt !== attempt) return;
      jobGateRef.current = stopDupesJob(jobGateRef.current);
      if (!aliveRef.current) return;
      setRunning(false);
      setCancelling(false);
      setRunError(String(error));
    }
  };

  const cancel = () => {
    if (!running || cancelling) return;
    setCancelling(true);
    void cancelDuplicates().catch(() => {});
  };

  const applyKeepRule = (rule: KeepRule, groups: readonly DuplicateGroup[]) => {
    if (!writable || useDiscardPile.getState().busy) return;
    const { keepIds, queueIds } = planAutoSelect(groups, rule);
    useDiscardPile.getState().updateBatch(queueIds, keepIds);
  };

  const visibleCount = page.key === filterKey ? page.count : DUPES_PAGE_SIZE;
  const visibleGroups = useMemo(
    () => (currentResult ? currentResult.groups.slice(0, visibleCount) : []),
    [currentResult, visibleCount],
  );
  const remainingGroups = currentResult
    ? Math.max(0, currentResult.groups.length - visibleCount)
    : 0;
  const nextBatchSize = Math.min(DUPES_PAGE_SIZE, remainingGroups);
  const showMore = () => {
    setPage({ key: filterKey, count: visibleCount + DUPES_PAGE_SIZE });
  };

  const candidateInfo = currentCandidates?.info;
  return {
    isLive,
    writable,
    name,
    setName,
    minSize,
    setMinSize,
    category,
    setCategory,
    includeHidden,
    setIncludeHidden,
    scope,
    clearScope,
    currentCandidates,
    candidateInfo,
    currentResult,
    visibleGroups,
    remainingGroups,
    nextBatchSize,
    showMore,
    running,
    cancelling,
    progress,
    progressPct: progressPercent(progress),
    canStart: !running && (candidateInfo === undefined || candidateInfo.files > 0),
    error: runError || currentCandidates?.error || null,
    start,
    cancel,
    applyKeepRule,
  };
}
