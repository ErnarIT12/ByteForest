/** Pure selection helpers for the Duplicate Finder view. */
import type { DuplicateCopy, DuplicateGroup } from './api';

export type KeepRule = 'oldest' | 'newest' | 'shortestPath';

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
