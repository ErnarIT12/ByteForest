import { describe, expect, it } from 'vitest';
import type { AutoSnapshot } from './api';
import type { RecentScan } from './recentScans';
import {
  canChangeDisk,
  canOpenSnapshot,
  compareDisabledReason,
  countSnapshots,
  formatRelativeTime,
  groupSnapshots,
  limitSnapshotRows,
  sameScanTarget,
  type ResultContext,
} from './snapshotsModel';

const NOW = Date.UTC(2026, 10, 24, 12, 0, 0);

const snap = (id: string, label: string, created: number): AutoSnapshot => ({
  id,
  label,
  created_unix_ms: created,
  node_count: 10,
  bytes_on_disk: 1000,
});

const scan = (target: string, label = target): RecentScan => ({ target, label, finishedAt: 0, bytes: 1 });

describe('formatRelativeTime', () => {
  it('uses minutes, hours and days', () => {
    expect(formatRelativeTime(NOW - 5_000, NOW)).toBe('just now');
    expect(formatRelativeTime(NOW - 5 * 60_000, NOW)).toBe('5 min ago');
    expect(formatRelativeTime(NOW - 2 * 3_600_000 - 1000, NOW)).toBe('2 h ago');
    expect(formatRelativeTime(NOW - 3 * 86_400_000, NOW)).toBe('3 d ago');
  });

  it('shows a date for old snapshots and tolerates a clock in the future', () => {
    expect(formatRelativeTime(NOW - 40 * 86_400_000, NOW)).toMatch(/2026/);
    expect(formatRelativeTime(NOW + 60_000, NOW)).toBe('just now');
    expect(formatRelativeTime(Number.NaN, NOW)).toBe('');
  });
});

describe('groupSnapshots', () => {
  it('lists recent targets first and attaches snapshots newest first', () => {
    const groups = groupSnapshots(
      [scan('D:'), scan('C:')],
      [snap('a', 'C:', 100), snap('b', 'D:', 50), snap('c', 'C:', 300), snap('d', 'c:', 200)],
    );
    expect(groups.map((g) => g.target)).toEqual(['D:', 'C:']);
    expect(groups[1].snapshots.map((s) => s.id)).toEqual(['c', 'd', 'a']);
    expect(groups[0].snapshots.map((s) => s.id)).toEqual(['b']);
  });

  it('adds targets that only have snapshots after the recent ones, newest first', () => {
    const groups = groupSnapshots(
      [scan('C:')],
      [snap('a', '/old', 10), snap('b', '/new', 90), snap('c', 'C:', 1)],
    );
    expect(groups.map((g) => g.target)).toEqual(['C:', '/new', '/old']);
    expect(groups[1].recent).toBeNull();
    expect(groups[1].label).toBe('new');
  });

  it('keeps recent scans without snapshots and handles empty input', () => {
    expect(groupSnapshots([scan('C:')], [])).toHaveLength(1);
    expect(groupSnapshots([], [])).toEqual([]);
  });

  it('breaks ties on the time by id', () => {
    const groups = groupSnapshots([], [snap('a', 'C:', 5), snap('b', 'C:', 5)]);
    expect(groups[0].snapshots.map((s) => s.id)).toEqual(['b', 'a']);
  });
});

describe('limitSnapshotRows', () => {
  const groups = groupSnapshots(
    [scan('C:'), scan('D:')],
    [snap('1', 'C:', 5), snap('2', 'C:', 4), snap('3', 'C:', 3), snap('4', 'D:', 2)],
  );

  it('cuts to the row budget but keeps recent scans', () => {
    const cut = limitSnapshotRows(groups, 2);
    expect(countSnapshots(cut)).toBe(2);
    expect(cut.map((g) => g.target)).toEqual(['C:', 'D:']);
    expect(cut[1].snapshots).toEqual([]);
  });

  it('shows everything for an infinite budget', () => {
    expect(countSnapshots(limitSnapshotRows(groups, Infinity))).toBe(4);
  });

  it('drops groups that have neither a recent scan nor rows left', () => {
    const only = groupSnapshots([], [snap('1', '/a', 2), snap('2', '/b', 1)]);
    expect(limitSnapshotRows(only, 1).map((g) => g.target)).toEqual(['/a']);
  });
});

describe('enable and disable rules', () => {
  const live: ResultContext = {
    scanning: false,
    resultScanId: 4,
    resultTarget: 'C:',
    resultSource: { kind: 'live' },
  };

  it('allows comparing a live result of the same target', () => {
    expect(compareDisabledReason(live, 'C:')).toBeNull();
    expect(compareDisabledReason(live, 'c:')).toBeNull();
  });

  it('refuses while scanning, without a result, for other targets and for snapshot views', () => {
    expect(compareDisabledReason({ ...live, scanning: true }, 'C:')).not.toBeNull();
    expect(compareDisabledReason({ ...live, resultScanId: null, resultTarget: null }, 'C:')).not.toBeNull();
    expect(compareDisabledReason(live, 'D:')).not.toBeNull();
    expect(
      compareDisabledReason({ ...live, resultSource: { kind: 'snapshot', created_unix_ms: 1 } }, 'C:'),
    ).not.toBeNull();
  });

  it('opens snapshots only while no scan runs', () => {
    expect(canOpenSnapshot({ scanning: false })).toBe(true);
    expect(canOpenSnapshot({ scanning: true })).toBe(false);
  });

  it('allows changing the disk only from a live result', () => {
    expect(canChangeDisk({ kind: 'live' })).toBe(true);
    expect(canChangeDisk({ kind: 'snapshot', created_unix_ms: 1 })).toBe(false);
  });

  it('compares targets without regard to case', () => {
    expect(sameScanTarget('C:\\Users', 'c:\\users')).toBe(true);
    expect(sameScanTarget('C:', 'D:')).toBe(false);
  });
});
