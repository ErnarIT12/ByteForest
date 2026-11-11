import { describe, expect, it } from 'vitest';
import type { Rect } from './api';
import { buildLayout, crumbsTo, headerFits, hitTest } from './treemapPaint';

const rect = (
  id: number,
  parent: number | null,
  depth: number,
  x: number,
  y: number,
  w: number,
  h: number,
  isDir = true,
): Rect => ({ id, parent, name: `n${id}`, size: w * h, x, y, w, h, depth, is_dir: isDir });

// Focused folder 10 with two top-level children; folder 11 holds file 13.
const rects = [
  rect(10, 1, 0, 0, 0, 100, 100),
  rect(11, 10, 1, 1, 1, 60, 98),
  rect(12, 10, 1, 61, 1, 38, 98, false),
  rect(13, 11, 2, 2, 20, 58, 78, false),
];

describe('treemap layout helpers', () => {
  it('gives every rect the palette slot of its top-level ancestor', () => {
    const layout = buildLayout(rects, 1, 16, 100, 100);
    expect(Array.from(layout.slot)).toEqual([-1, 0, 1, 0]);
    expect(layout.index.get(13)).toBe(3);
  });

  it('hit-tests the deepest rect', () => {
    const layout = buildLayout(rects, 1, 16, 100, 100);
    expect(hitTest(layout, 10, 50)).toBe(3);
    expect(hitTest(layout, 10, 5)).toBe(1);
    expect(hitTest(layout, 80, 50)).toBe(2);
    expect(hitTest(layout, 0, 0)).toBe(0);
    expect(hitTest(layout, 200, 0)).toBe(-1);
  });

  it('extends the focus path down to a rect', () => {
    const layout = buildLayout(rects, 1, 16, 100, 100);
    const focus = [
      { id: 1, name: 'C:' },
      { id: 10, name: 'n10' },
    ];
    expect(crumbsTo(layout, 3, focus).map((c) => c.id)).toEqual([1, 10, 11, 13]);
    expect(crumbsTo(layout, 0, focus)).toEqual(focus);
  });

  it('mirrors bf_core::header_fits', () => {
    expect(headerFits(48, 48, 16)).toBe(true);
    expect(headerFits(47.9, 100, 16)).toBe(false);
    expect(headerFits(100, 100, 0)).toBe(false);
  });
});
