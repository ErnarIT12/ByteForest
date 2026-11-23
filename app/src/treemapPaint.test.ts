import { describe, expect, it } from 'vitest';
import type { Rect } from './api';
import { contrastRatio } from './contrast';
import {
  CATEGORIES,
  buildLayout,
  categoryColor,
  crumbsTo,
  headerFits,
  hatchLines,
  hitTest,
  labelColors,
  type Role,
  type Theme,
} from './treemapPaint';

const rect = (
  id: number,
  parent: number | null,
  depth: number,
  x: number,
  y: number,
  w: number,
  h: number,
  isDir = true,
): Rect => ({ id, parent, name: `n${id}`, size: w * h, x, y, w, h, depth, is_dir: isDir, category: isDir ? null : 'other' });

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

const themes: Theme[] = ['dark', 'light'];
const saturation = (hex: string) => {
  const [r, g, b] = [1, 3, 5].map((i) => Number.parseInt(hex.slice(i, i + 2), 16));
  return Math.max(r, g, b) - Math.min(r, g, b);
};

describe('category colour mode', () => {
  it('gives every category its own file colour in both themes', () => {
    for (const theme of themes) {
      const fills = CATEGORIES.map((c) => categoryColor(theme, c, 'file'));
      expect(new Set(fills).size).toBe(CATEGORIES.length);
    }
  });

  it('keeps folders neutral and independent of the category', () => {
    const roles: Role[] = ['dirFill', 'dirStroke', 'header'];
    for (const theme of themes) {
      for (const role of roles) {
        const colors = new Set(CATEGORIES.map((c) => categoryColor(theme, c, role)));
        expect(colors.size).toBe(1);
        expect(saturation([...colors][0])).toBeLessThan(
          saturation(categoryColor(theme, 'video', 'file')),
        );
        expect(saturation([...colors][0])).toBeLessThan(20);
      }
    }
  });

  it('keeps file labels readable on every category fill', () => {
    for (const theme of themes) {
      for (const category of CATEGORIES) {
        const fill = categoryColor(theme, category, 'file');
        const { main } = labelColors(fill);
        expect(contrastRatio(fill, main), `${theme} ${category}`).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});

describe('hatchLines', () => {
  it('covers the whole rectangle with evenly spaced diagonals', () => {
    const lines = hatchLines(10, 20, 30, 12, 6);
    expect(lines.length).toBe(Math.ceil((30 + 12) / 6) - 1);
    expect(lines[0]).toEqual([16, 20, 10, 26]);
    // Every line runs from the top edge line to the left edge line, one gap further each time.
    lines.forEach(([x1, y1, x2, y2], i) => {
      expect(x1 - 10).toBe(6 * (i + 1));
      expect(y1).toBe(20);
      expect(x2).toBe(10);
      expect(y2 - 20).toBe(6 * (i + 1));
    });
  });

  it('draws nothing for empty rectangles or a bad gap', () => {
    expect(hatchLines(0, 0, 0, 10, 4)).toEqual([]);
    expect(hatchLines(0, 0, 10, 10, 0)).toEqual([]);
  });
});
