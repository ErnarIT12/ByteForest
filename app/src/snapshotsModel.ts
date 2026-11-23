/** Pure rules behind the automatic snapshots UI: grouping, relative dates and what is allowed. */
import type { AutoSnapshot, ResultSource } from './api';
import { formatUnixMs } from './format';
import type { RecentScan } from './recentScans';
import { folderLabel } from './scanTargets';

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** "just now", "5 min ago", "2 h ago", "3 d ago"; older than 30 days a plain date. */
export function formatRelativeTime(ms: number, now: number): string {
  if (!Number.isFinite(ms) || !Number.isFinite(now)) return '';
  const age = now - ms;
  if (age < MINUTE) return 'just now';
  if (age < HOUR) return `${Math.floor(age / MINUTE)} min ago`;
  if (age < DAY) return `${Math.floor(age / HOUR)} h ago`;
  if (age < 30 * DAY) return `${Math.floor(age / DAY)} d ago`;
  return formatUnixMs(ms);
}

const fullFormat = new Intl.DateTimeFormat('en-US', {
  year: 'numeric',
  month: 'short',
  day: 'numeric',
  hour: '2-digit',
  minute: '2-digit',
});

/** Absolute date and time of a unix time in ms ("Nov 15, 2026, 02:30 PM"), "" when unusable. */
export function formatUnixMsFull(ms: number): string {
  if (!Number.isFinite(ms)) return '';
  const d = new Date(ms);
  return Number.isNaN(d.getTime()) ? '' : fullFormat.format(d);
}

/** Targets are drive roots and paths; they compare without regard to case. */
export const sameScanTarget = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();

/** A scan target with the snapshots saved for it, newest first. */
export interface SnapshotGroup {
  target: string;
  /** Short name: "C:" or the folder's name. */
  label: string;
  /** The recent scan of this target, if there is one (it can be rescanned in one click). */
  recent: RecentScan | null;
  snapshots: AutoSnapshot[];
}

const shortName = (target: string): string =>
  /^[A-Za-z]:$/.test(target) ? target : folderLabel(target);

/**
 * Recent scans and saved snapshots merged by target. Targets of recent scans come first in
 * their order (newest scan first), then targets that only have snapshots, the one with the
 * newest snapshot first. Within a group the snapshots are newest first.
 */
export function groupSnapshots(
  recent: readonly RecentScan[],
  snapshots: readonly AutoSnapshot[],
): SnapshotGroup[] {
  const groups: SnapshotGroup[] = [];
  const find = (target: string) => groups.find((g) => sameScanTarget(g.target, target));
  for (const scan of recent) {
    if (!find(scan.target)) {
      groups.push({
        target: scan.target,
        label: scan.label || shortName(scan.target),
        recent: scan,
        snapshots: [],
      });
    }
  }
  const newestFirst = [...snapshots].sort(
    (a, b) => b.created_unix_ms - a.created_unix_ms || (a.id < b.id ? 1 : a.id > b.id ? -1 : 0),
  );
  for (const snap of newestFirst) {
    let group = find(snap.label);
    if (!group) {
      group = { target: snap.label, label: shortName(snap.label), recent: null, snapshots: [] };
      groups.push(group);
    }
    group.snapshots.push(snap);
  }
  return groups;
}

/** Number of snapshot rows in `groups`. */
export const countSnapshots = (groups: readonly SnapshotGroup[]): number =>
  groups.reduce((n, g) => n + g.snapshots.length, 0);

/**
 * The groups cut to at most `max` snapshot rows overall. Groups keep their order; one left
 * without snapshots stays when it has a recent scan and goes otherwise.
 */
export function limitSnapshotRows(groups: readonly SnapshotGroup[], max: number): SnapshotGroup[] {
  let left = max;
  const out: SnapshotGroup[] = [];
  for (const group of groups) {
    const snapshots = group.snapshots.slice(0, Math.max(0, left));
    left -= snapshots.length;
    if (snapshots.length > 0 || group.recent) out.push({ ...group, snapshots });
  }
  return out;
}

/** The pieces of app state the rules below depend on. */
export interface ResultContext {
  scanning: boolean;
  resultScanId: number | null;
  resultTarget: string | null;
  resultSource: ResultSource;
}

/** A snapshot can be opened while no scan is running. */
export const canOpenSnapshot = (ctx: Pick<ResultContext, 'scanning'>): boolean => !ctx.scanning;

/** Why "Compare with Current" is unavailable, or null when it is available. */
export function compareDisabledReason(ctx: ResultContext, snapshotLabel: string): string | null {
  if (ctx.scanning) return 'A scan is running.';
  if (ctx.resultScanId === null || ctx.resultTarget === null) return 'Scan something first.';
  if (ctx.resultSource.kind !== 'live') return 'Rescan to compare.';
  if (!sameScanTarget(ctx.resultTarget, snapshotLabel)) {
    return 'The current result is a different location.';
  }
  return null;
}

/** Whether items of the shown result may be moved to the trash or queued for it. */
export const canChangeDisk = (source: ResultSource): boolean => source.kind === 'live';
