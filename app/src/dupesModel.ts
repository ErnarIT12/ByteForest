/** Pure selection, job-gating, and pagination helpers for the Duplicate Finder view. */
import type { DuplicateCopy, DuplicateGroup, HashProgress } from './api';

export type KeepRule = 'oldest' | 'newest' | 'shortestPath';

/** Number of duplicate groups rendered per batch before clicking "Show more". */
export const DUPES_PAGE_SIZE = 50;

/**
 * Tracks the active duplicate-hashing job so out-of-order IPC resolves, cancelled attempts,
 * and stale events from superseded jobs or older tree generations are handled deterministically.
 */
export interface DupesJobGate {
  /** Monotonic counter bumped on every start or stop to ignore superseded `startDuplicates` resolves. */
  attempt: number;
  /** True between calling `startDuplicates` and receiving its `job_id` (or its completion event). */
  pending: boolean;
  jobId: number | null;
  lastKnown: number;
  lastFinished: number;
  filterKey: string;
  /** Tree `generation` captured before `startDuplicates` was invoked. */
  startedGeneration: number;
}

export function createDupesJobGate(): DupesJobGate {
  return {
    attempt: 0,
    pending: false,
    jobId: null,
    lastKnown: 0,
    lastFinished: 0,
    filterKey: '',
    startedGeneration: 0,
  };
}

/** Prepares the gate for a new `startDuplicates` call, capturing `generation` before hashing starts. */
export function beginDupesJob(
  gate: DupesJobGate,
  filterKey: string,
  generation: number,
): DupesJobGate {
  return {
    attempt: gate.attempt + 1,
    pending: true,
    jobId: null,
    lastKnown: gate.lastKnown,
    lastFinished: gate.lastFinished,
    filterKey,
    startedGeneration: generation,
  };
}

/** Stops tracking the active job (e.g. when filters change), invalidating any in-flight invoke. */
export function stopDupesJob(gate: DupesJobGate): DupesJobGate {
  return {
    ...gate,
    attempt: gate.attempt + 1,
    pending: false,
    jobId: null,
    filterKey: '',
  };
}

/** Whether a backend event tagged with `jobId` belongs to the job the UI is currently waiting for. */
export function acceptDupesJob(gate: DupesJobGate, jobId: number): boolean {
  if (gate.pending) return jobId > gate.lastKnown;
  return gate.jobId !== null && jobId === gate.jobId;
}

/**
 * Records the `jobId` returned by `startDuplicates`. Returns whether the job is still active
 * (false if the attempt was superseded or its terminal event already arrived).
 */
export function resolveDupesJob(
  gate: DupesJobGate,
  attempt: number,
  jobId: number,
): { gate: DupesJobGate; active: boolean; superseded: boolean } {
  const lastKnown = Math.max(gate.lastKnown, jobId);
  if (gate.attempt !== attempt) {
    return { gate: { ...gate, lastKnown }, active: false, superseded: true };
  }
  const alreadyFinished = gate.lastFinished === jobId;
  return {
    gate: {
      ...gate,
      lastKnown,
      jobId: alreadyFinished ? null : jobId,
      pending: false,
    },
    active: !alreadyFinished,
    superseded: false,
  };
}

/** Marks a terminal event (`done` or `error`) for an accepted `jobId`. */
export function finishDupesJob(gate: DupesJobGate, jobId: number): DupesJobGate {
  return {
    ...gate,
    pending: false,
    jobId: null,
    lastKnown: Math.max(gate.lastKnown, jobId),
    lastFinished: jobId,
  };
}

/** Completion percentage (0..100) for a hashing progress snapshot. */
export function progressPercent(progress: HashProgress | null): number {
  if (!progress) return 0;
  if (progress.bytes_total > 0) {
    return Math.min(100, Math.round((progress.bytes_done * 100) / progress.bytes_total));
  }
  if (progress.files_total > 0) {
    return Math.min(100, Math.round((progress.files_done * 100) / progress.files_total));
  }
  return 0;
}

function hasKnownMtime(copy: DuplicateCopy): boolean {
  return Number.isFinite(copy.mtime) && copy.mtime > 0;
}

/** Deterministic tie-breaker: shorter path, then lexicographical path, then smallest id. */
function compareTieBreak(a: DuplicateCopy, b: DuplicateCopy): number {
  if (a.path.length !== b.path.length) return a.path.length - b.path.length;
  if (a.path !== b.path) return a.path < b.path ? -1 : 1;
  return a.id - b.id;
}

function compareByRule(a: DuplicateCopy, b: DuplicateCopy, rule: KeepRule): number {
  if (rule === 'shortestPath') {
    return compareTieBreak(a, b);
  }
  const aKnown = hasKnownMtime(a);
  const bKnown = hasKnownMtime(b);
  if (aKnown !== bKnown) return aKnown ? -1 : 1;
  if (aKnown && bKnown && a.mtime !== b.mtime) {
    return rule === 'oldest' ? a.mtime - b.mtime : b.mtime - a.mtime;
  }
  return compareTieBreak(a, b);
}

/**
 * Picks the single copy to keep according to `rule`, or `null` when `copies` is empty.
 *
 * - `oldest`: smallest positive `mtime` (unknown `mtime <= 0` sorts last).
 * - `newest`: largest positive `mtime` (unknown `mtime <= 0` sorts last).
 * - `shortestPath`: shortest `path.length`.
 *
 * Ties always break by shorter path, then lexicographical path, then smallest `id`.
 */
export function pickCopyToKeep(
  copies: readonly DuplicateCopy[],
  rule: KeepRule,
): DuplicateCopy | null {
  if (copies.length === 0) return null;
  let best = copies[0];
  for (let i = 1; i < copies.length; i += 1) {
    const candidate = copies[i];
    if (compareByRule(candidate, best, rule) < 0) {
      best = candidate;
    }
  }
  return best;
}

/**
 * Computes which copy ids to keep and which extra copies to queue for cleanup across `groups`.
 * Every group with at least two copies keeps one copy and queues the rest; never queues all
 * copies of any group.
 */
export function planAutoSelect(
  groups: readonly DuplicateGroup[],
  rule: KeepRule,
): { keepIds: number[]; queueIds: number[] } {
  const keepIds: number[] = [];
  const queueIds: number[] = [];
  for (const group of groups) {
    if (group.copies.length < 2) continue;
    const keep = pickCopyToKeep(group.copies, rule);
    if (!keep) continue;
    keepIds.push(keep.id);
    for (const copy of group.copies) {
      if (copy.id !== keep.id) {
        queueIds.push(copy.id);
      }
    }
  }
  return { keepIds, queueIds };
}

/** Whether every copy in `group` is currently queued for cleanup. */
export function allCopiesQueued(group: DuplicateGroup, queued: ReadonlySet<number>): boolean {
  return group.copies.length > 0 && group.copies.every((copy) => queued.has(copy.id));
}
