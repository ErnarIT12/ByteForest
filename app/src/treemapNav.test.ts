import { describe, expect, it } from 'vitest';
import type { Rect } from './api';
import { largestChild, nearestSibling } from './treemapNav';

const rect = (
  id: number,
  parent: number | null,
  x: number,
  y: number,
  w: number,
  h: number,
): Rect => ({
  id,
  parent,
  name: `n${id}`,
  size: w * h,
  x,
  y,
  w,
  h,
  depth: parent === null ? 0 : 1,
  is_dir: false,
  category: null,
});

// Root 10 split: 11 left half, 12 top right, 13 bottom right; 14 is nested inside 11.
const rects = [
  rect(10, null, 0, 0, 100, 100),
  rect(11, 10, 0, 0, 50, 100),
  rect(12, 10, 50, 0, 50, 60),
  rect(13, 10, 50, 60, 50, 40),
  rect(14, 11, 0, 0, 20, 20),
];

describe('treemap keyboard navigation', () => {
  it('moves to the sibling in the arrow direction', () => {
    expect(nearestSibling(rects, 1, 'right')).toBe(2);
    expect(nearestSibling(rects, 2, 'down')).toBe(3);
    expect(nearestSibling(rects, 3, 'up')).toBe(2);
    expect(nearestSibling(rects, 3, 'left')).toBe(1);
  });

  it('stays put when nothing lies in that direction', () => {
    expect(nearestSibling(rects, 1, 'left')).toBe(-1);
    expect(nearestSibling(rects, 2, 'up')).toBe(-1);
  });

  it('ignores rects of other parents and the root', () => {
    expect(nearestSibling(rects, 4, 'right')).toBe(-1);
    expect(nearestSibling(rects, 0, 'right')).toBe(-1);
  });

  it('prefers the same row over a nearer off-axis rect', () => {
    const row = [
      rect(1, null, 0, 0, 100, 100),
      rect(2, 1, 0, 40, 10, 20),
      rect(3, 1, 30, 40, 10, 20),
      rect(4, 1, 15, 80, 10, 20),
    ];
    expect(nearestSibling(row, 1, 'right')).toBe(2);
  });

  it('finds the largest direct child', () => {
    expect(largestChild(rects)).toBe(1);
    expect(largestChild(rects, 1)).toBe(4);
    expect(largestChild(rects, 4)).toBe(-1);
    expect(largestChild([])).toBe(-1);
  });
});
