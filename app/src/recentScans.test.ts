import { describe, expect, it } from 'vitest';
import { MAX_RECENT_SCANS, addRecentScan, parseRecentScans, type RecentScan } from './recentScans';

const scan = (target: string, finishedAt = 1, bytes = 100): RecentScan => ({
  target,
  label: target,
  finishedAt,
  bytes,
});

describe('addRecentScan', () => {
  it('puts the newest scan first', () => {
    const list = addRecentScan([scan('C:')], scan('D:'));
    expect(list.map((s) => s.target)).toEqual(['D:', 'C:']);
  });

  it('replaces an older entry of the same target', () => {
    const list = addRecentScan([scan('D:'), scan('C:', 1, 5)], scan('c:', 2, 9));
    expect(list.map((s) => s.target)).toEqual(['c:', 'D:']);
    expect(list[0].bytes).toBe(9);
  });

  it('keeps at most the cap, dropping the oldest', () => {
    let list: RecentScan[] = [];
    for (let i = 0; i < MAX_RECENT_SCANS + 3; i += 1) list = addRecentScan(list, scan(`/f${i}`));
    expect(list).toHaveLength(MAX_RECENT_SCANS);
    expect(list[0].target).toBe(`/f${MAX_RECENT_SCANS + 2}`);
    expect(list.at(-1)?.target).toBe('/f3');
  });

  it('does not change the input list', () => {
    const input = [scan('C:')];
    addRecentScan(input, scan('D:'));
    expect(input).toHaveLength(1);
  });
});

describe('parseRecentScans', () => {
  it('reads what was stored, in order', () => {
    const stored = JSON.stringify([scan('D:'), scan('C:')]);
    expect(parseRecentScans(stored).map((s) => s.target)).toEqual(['D:', 'C:']);
  });

  it('survives missing, broken and wrongly shaped data', () => {
    expect(parseRecentScans(null)).toEqual([]);
    expect(parseRecentScans('{oops')).toEqual([]);
    expect(parseRecentScans('{"a":1}')).toEqual([]);
    expect(parseRecentScans('[1,null,{"target":"C:"}]')).toEqual([]);
  });

  it('drops malformed entries and duplicates, and applies the cap', () => {
    const many = Array.from({ length: 12 }, (_, i) => scan(`/f${i}`));
    const stored = JSON.stringify([{ target: 5 }, scan('/f0'), ...many]);
    const list = parseRecentScans(stored);
    expect(list).toHaveLength(MAX_RECENT_SCANS);
    expect(list[0].target).toBe('/f0');
    expect(new Set(list.map((s) => s.target)).size).toBe(list.length);
  });
});
