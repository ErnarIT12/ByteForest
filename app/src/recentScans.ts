/** Recently finished scans, kept in localStorage so the sidebar can rescan them in one click. */

export interface RecentScan {
  /** Exactly what was passed to `startScan`: "C:" or a folder path. */
  target: string;
  /** Short name for the list, such as "C:" or the folder name. */
  label: string;
  /** When the scan finished (ms since the epoch). */
  finishedAt: number;
  /** Total bytes of the result. */
  bytes: number;
}

/** Most entries kept. */
export const MAX_RECENT_SCANS = 8;
export const RECENT_SCANS_KEY = 'byteforest.recentScans';

const sameTarget = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

/** `entry` first, the same target's older entry dropped, at most `MAX_RECENT_SCANS` kept. */
export function addRecentScan(list: readonly RecentScan[], entry: RecentScan): RecentScan[] {
  return [entry, ...list.filter((item) => !sameTarget(item.target, entry.target))].slice(
    0,
    MAX_RECENT_SCANS,
  );
}

function isRecentScan(value: unknown): value is RecentScan {
  if (typeof value !== 'object' || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v.target === 'string' &&
    v.target !== '' &&
    typeof v.label === 'string' &&
    typeof v.finishedAt === 'number' &&
    Number.isFinite(v.finishedAt) &&
    typeof v.bytes === 'number' &&
    Number.isFinite(v.bytes) &&
    v.bytes >= 0
  );
}

/** Entries of a stored JSON value; anything malformed is dropped, never thrown on. */
export function parseRecentScans(raw: string | null): RecentScan[] {
  if (raw === null) return [];
  let value: unknown;
  try {
    value = JSON.parse(raw);
  } catch {
    return [];
  }
  if (!Array.isArray(value)) return [];
  let list: RecentScan[] = [];
  // Oldest first, so that addRecentScan keeps the stored order and drops duplicates.
  for (const item of [...value].reverse()) {
    if (isRecentScan(item)) {
      list = addRecentScan(list, {
        target: item.target,
        label: item.label,
        finishedAt: item.finishedAt,
        bytes: item.bytes,
      });
    }
  }
  return list;
}

export function loadRecentScans(): RecentScan[] {
  try {
    return parseRecentScans(localStorage.getItem(RECENT_SCANS_KEY));
  } catch {
    return [];
  }
}

export function saveRecentScans(list: readonly RecentScan[]): void {
  try {
    localStorage.setItem(RECENT_SCANS_KEY, JSON.stringify(list));
  } catch {
    /* The list then lasts for this session only. */
  }
}
