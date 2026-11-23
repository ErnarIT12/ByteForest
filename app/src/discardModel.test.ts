import { describe, expect, it } from 'vitest';
import { addIds, pruneIds, removeId, removeIds, survivingSelection } from './discardModel';

describe('addIds', () => {
  it('appends new ids once and keeps the order', () => {
    expect(addIds([3], [5, 3, 7, 5], 0)).toEqual([3, 5, 7]);
  });

  it('never queues the root', () => {
    expect(addIds([], [0, 4], 0)).toEqual([4]);
  });

  it('returns the same array when nothing changed', () => {
    const ids = [1, 2];
    expect(addIds(ids, [2, 1], 0)).toBe(ids);
    expect(addIds(ids, [0], 0)).toBe(ids);
  });

  it('stops at the limit', () => {
    expect(addIds([1], [2, 3, 4], null, 3)).toEqual([1, 2, 3]);
  });
});

describe('removeId', () => {
  it('drops one id', () => {
    expect(removeId([1, 2, 3], 2)).toEqual([1, 3]);
  });

  it('keeps the array when the id is not queued', () => {
    const ids = [1, 2];
    expect(removeId(ids, 9)).toBe(ids);
    expect(removeIds(ids, new Set([9]))).toBe(ids);
    expect(removeIds(ids, new Set([1, 9]))).toEqual([2]);
  });
});

describe('pruneIds', () => {
  it('keeps only ids the summary lists, in queue order', () => {
    const summary = { items: [{ id: 9 }, { id: 2 }] };
    expect(pruneIds([2, 5, 9, 7], summary)).toEqual([2, 9]);
  });

  it('returns the same array when nothing vanished', () => {
    const ids = [2, 9];
    expect(pruneIds(ids, { items: [{ id: 9 }, { id: 2 }] })).toBe(ids);
  });

  it('empties the queue for an empty summary', () => {
    expect(pruneIds([1, 2], { items: [] })).toEqual([]);
  });
});

describe('survivingSelection', () => {
  it('picks the node above the first removed link', () => {
    expect(survivingSelection([0, 4, 8, 12], new Set([8]))).toBe(4);
    expect(survivingSelection([0, 4, 8, 12], new Set([12, 4]))).toBe(0);
  });

  it('is null when nothing on the chain was removed or the chain is unknown', () => {
    expect(survivingSelection([0, 4], new Set([9]))).toBeNull();
    expect(survivingSelection(null, new Set([4]))).toBeNull();
  });
});
