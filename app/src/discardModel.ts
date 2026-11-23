/** Pure helpers behind the Discard Pile: the queue of node ids and the selection after a batch. */
import { MAX_DISCARD_IDS } from './api';

/**
 * `ids` plus the new `add` ids, without duplicates and without `rootId` (the scanned root
 * cannot be queued). Returns `ids` itself when nothing was added. Capped at `limit` entries.
 */
export function addIds(
  ids: readonly number[],
  add: readonly number[],
  rootId: number | null,
  limit = MAX_DISCARD_IDS,
): readonly number[] {
  const seen = new Set(ids);
  const next = [...ids];
  for (const id of add) {
    if (id === rootId || seen.has(id) || next.length >= limit) continue;
    seen.add(id);
    next.push(id);
  }
  return next.length === ids.length ? ids : next;
}

/** `ids` without `id`; returns `ids` itself when it is not queued. */
export function removeId(ids: readonly number[], id: number): readonly number[] {
  return ids.includes(id) ? ids.filter((queued) => queued !== id) : ids;
}

/** `ids` without `gone`; returns `ids` itself when none of them is queued. */
export function removeIds(ids: readonly number[], gone: ReadonlySet<number>): readonly number[] {
  return ids.some((id) => gone.has(id)) ? ids.filter((id) => !gone.has(id)) : ids;
}

/**
 * Keeps the queued ids the backend still lists (it drops removed, invalid and nested ones),
 * in queue order. Returns `ids` itself when nothing vanished.
 */
export function pruneIds(
  ids: readonly number[],
  summary: { items: readonly { id: number }[] },
): readonly number[] {
  const alive = new Set(summary.items.map((item) => item.id));
  return ids.every((id) => alive.has(id)) ? ids : ids.filter((id) => alive.has(id));
}

/**
 * The selection after removing `removed` nodes. `chain` is the path root..selection taken
 * before the removal: the node just above the first removed link survives. Null when the
 * selection is unaffected or unknown.
 */
export function survivingSelection(
  chain: readonly number[] | null,
  removed: ReadonlySet<number>,
): number | null {
  if (!chain) return null;
  const hit = chain.findIndex((id) => removed.has(id));
  return hit > 0 ? chain[hit - 1] : null;
}
