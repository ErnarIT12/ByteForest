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
  paintTreemapDiff,
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

describe('treemap change marks', () => {
  const colors = { grow: '#9a3412', shrink: '#17663a', halo: '#ffffff' };
  const stub = () => {
    const calls: string[] = [];
    const ctx = new Proxy(
      {},
      {
        get: (_t, prop: string) => {
          if (prop === 'measureText') return (t: string) => ({ width: t.length * 6 });
          return (...args: unknown[]) => calls.push(`${prop}:${args.join(',')}`);
        },
        set: (_t, prop: string, value: unknown) => {
          calls.push(`${prop}=${String(value)}`);
          return true;
        },
      },
    ) as unknown as CanvasRenderingContext2D;
    return { ctx, calls };
  };
  const changed = (status: 'added' | 'grown' | 'shrunk', delta: number) => ({
    status,
    base: status === 'added' ? null : 10,
    delta,
    removed: null,
  });

  it('draws nothing while comparing finds no change', () => {
    const { ctx, calls } = stub();
    paintTreemapDiff(ctx, buildLayout(rects, 1, 16, 100, 100), colors);
    expect(calls).toEqual([]);
  });

  it('outlines a change on a halo and puts the delta pill on tall tiles', () => {
    const marked = rects.map((r) =>
      r.id === 12 ? { ...r, w: 80, diff: changed('grown', 2048) } : r,
    );
    const { ctx, calls } = stub();
    paintTreemapDiff(ctx, buildLayout(marked, 1, 16, 100, 100), colors);
    expect(calls.filter((c) => c.startsWith('strokeRect'))).toHaveLength(2);
    expect(calls).toContain('strokeStyle=#ffffff');
    expect(calls).toContain('strokeStyle=#9a3412');
    expect(calls.some((c) => c.startsWith('fillText:+2.0 KB'))).toBe(true);
  });

  it('uses a dashed outline for shrunk tiles and a dotted one for new tiles', () => {
    const run = (status: 'shrunk' | 'added') => {
      const marked = rects.map((r) => (r.id === 12 ? { ...r, diff: changed(status, -5) } : r));
      const { ctx, calls } = stub();
      paintTreemapDiff(ctx, buildLayout(marked, 1, 16, 100, 100), colors);
      return calls.filter((c) => c.startsWith('setLineDash') && c !== 'setLineDash:').pop();
    };
    expect(run('shrunk')).toBe('setLineDash:8,4');
    expect(run('added')).toBe('setLineDash:0.01,3.5');
  });

  it('thins the outline of folders that only hold changes', () => {
    const marked = rects.map((r) =>
      r.id === 11 ? { ...r, diff: changed('grown', 9) } : r.id === 13 ? { ...r, diff: changed('grown', 9) } : r,
    );
    const { ctx, calls } = stub();
    paintTreemapDiff(ctx, buildLayout(marked, 1, 16, 100, 100), colors);
    // Folder 11 is the thin level-1 mark (1 px), file 13 the strong one (2 px plus 2 halo).
    const widths = calls.filter((c) => c.startsWith('lineWidth=')).map((c) => c.slice(10));
    expect(widths).toContain('1');
    expect(widths).toContain('2');
    expect(widths).toContain('4');
  });

  it('fills tiles too small for an outline with the mark colour', () => {
    const marked = [
      rects[0],
      { ...rect(11, 10, 1, 1, 1, 4, 4, false), diff: changed('grown', 1) },
    ];
    const { ctx, calls } = stub();
    paintTreemapDiff(ctx, buildLayout(marked, 1, 16, 100, 100), colors);
    expect(calls).toContain('fillStyle=#9a3412');
    expect(calls.filter((c) => c.startsWith('strokeRect'))).toHaveLength(0);
  });
});
