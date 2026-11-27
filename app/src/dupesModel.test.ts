import { describe, expect, it } from 'vitest';
import type { DuplicateCopy, DuplicateGroup } from './api';
import { allCopiesQueued, pickCopyToKeep, planAutoSelect } from './dupesModel';

const copy = (id: number, path: string, mtime: number): DuplicateCopy => ({
  id,
  name: path.split(/[/\\]/).pop() ?? path,
  path,
  mtime,
  category: 'other',
});

const group = (size: number, copies: DuplicateCopy[]): DuplicateGroup => ({
  size,
  reclaimable: size * Math.max(0, copies.length - 1),
  hash: '0'.repeat(64),
  copies,
});

describe('pickCopyToKeep', () => {
  it('returns null for an empty list', () => {
    expect(pickCopyToKeep([], 'oldest')).toBeNull();
    expect(pickCopyToKeep([], 'newest')).toBeNull();
    expect(pickCopyToKeep([], 'shortestPath')).toBeNull();
  });

  it('picks the smallest positive mtime for oldest and treats <= 0 as unknown/last', () => {
    const unknown = copy(1, '/a', 0);
    const negative = copy(2, '/b', -10);
    const older = copy(3, '/very/long/path/file.bin', 100);
    const newer = copy(4, '/c', 200);
    expect(pickCopyToKeep([unknown, negative, newer, older], 'oldest')?.id).toBe(3);
  });

  it('picks the largest mtime for newest and treats <= 0 as unknown/last', () => {
    const unknown = copy(1, '/a', 0);
    const older = copy(2, '/b', 100);
    const newer = copy(3, '/very/long/path/file.bin', 200);
    expect(pickCopyToKeep([unknown, older, newer], 'newest')?.id).toBe(3);
  });

  it('breaks oldest and newest ties by shorter path, then lexicographical path, then smallest id', () => {
    const longPath = copy(1, '/folder/sub/a.bin', 500);
    const shortB = copy(2, '/b/file.bin', 500);
    const shortAHighId = copy(9, '/a/file.bin', 500);
    const shortALowId = copy(4, '/a/file.bin', 500);
    expect(pickCopyToKeep([longPath, shortB, shortAHighId, shortALowId], 'oldest')?.id).toBe(4);
    expect(pickCopyToKeep([longPath, shortB, shortAHighId, shortALowId], 'newest')?.id).toBe(4);

    // When all mtimes are unknown (<= 0), tie-breaker decides deterministically.
    const u1 = copy(5, '/z/x.bin', 0);
    const u2 = copy(6, '/a/x.bin', 0);
    expect(pickCopyToKeep([u1, u2], 'oldest')?.id).toBe(6);
    expect(pickCopyToKeep([u1, u2], 'newest')?.id).toBe(6);
  });

  it('picks shortest path regardless of mtime and breaks ties lexicographically then by id', () => {
    const olderLong = copy(1, '/archive/2020/file.bin', 10);
    const shortZ = copy(2, '/z/f.bin', 999);
    const shortAHigh = copy(8, '/a/f.bin', 500);
    const shortALow = copy(3, '/a/f.bin', 800);
    expect(pickCopyToKeep([olderLong, shortZ, shortAHigh, shortALow], 'shortestPath')?.id).toBe(3);
  });
});

describe('planAutoSelect', () => {
  it('keeps exactly one copy per multi-copy group and queues the rest', () => {
    const g1 = group(1024, [copy(10, '/long/path/a.bin', 300), copy(11, '/a.bin', 100), copy(12, '/mid/a.bin', 200)]);
    const g2 = group(2048, [copy(20, '/b1.bin', 50), copy(21, '/b2.bin', 90)]);
    const single = group(512, [copy(30, '/solo.bin', 10)]);

    const oldest = planAutoSelect([g1, g2, single], 'oldest');
    expect(oldest.keepIds).toEqual([11, 20]);
    expect(oldest.queueIds).toEqual([10, 12, 21]);

    const newest = planAutoSelect([g1, g2, single], 'newest');
    expect(newest.keepIds).toEqual([10, 21]);
    expect(newest.queueIds).toEqual([11, 12, 20]);

    const shortest = planAutoSelect([g1, g2, single], 'shortestPath');
    expect(shortest.keepIds).toEqual([11, 20]);
    expect(shortest.queueIds).toEqual([10, 12, 21]);
  });

  it('never queues all copies of any group', () => {
    const groups = [
      group(100, [copy(1, '/a', 1), copy(2, '/b', 1)]),
      group(200, [copy(3, '/c', 0), copy(4, '/d', 0), copy(5, '/e', 0)]),
    ];
    for (const rule of ['oldest', 'newest', 'shortestPath'] as const) {
      const { queueIds } = planAutoSelect(groups, rule);
      const queued = new Set(queueIds);
      for (const g of groups) {
        expect(allCopiesQueued(g, queued)).toBe(false);
      }
    }
  });
});

describe('allCopiesQueued', () => {
  it('returns false for empty groups and partially queued groups, true only when every copy is queued', () => {
    const empty = group(100, []);
    const pair = group(100, [copy(1, '/a', 1), copy(2, '/b', 2)]);
    expect(allCopiesQueued(empty, new Set([1, 2]))).toBe(false);
    expect(allCopiesQueued(pair, new Set())).toBe(false);
    expect(allCopiesQueued(pair, new Set([1]))).toBe(false);
    expect(allCopiesQueued(pair, new Set([1, 2]))).toBe(true);
    expect(allCopiesQueued(pair, new Set([1, 2, 3]))).toBe(true);
  });
});
