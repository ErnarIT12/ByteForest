import { describe, expect, it, vi } from 'vitest';
import { OTHER_ARC_ID, type SunburstArc } from './api';
import {
  arcColor,
  arcRadii,
  buildSunburstLayout,
  isNodeArc,
  labelRoom,
  normalizeAngle,
  paintSunburst,
  paintSunburstHighlights,
  sunburstCrumbsTo,
  sunburstGeometry,
  sunburstHitTest,
} from './sunburstPaint';
import { categoryColor, treemapColor } from './treemapPaint';

const TAU = Math.PI * 2;

const arc = (
  id: number,
  parent: number,
  depth: number,
  start: number,
  end: number,
  isDir = true,
  extra: Partial<SunburstArc> = {},
): SunburstArc => ({
  id,
  parent,
  name: `n${id}`,
  size: Math.round((end - start) * 1000),
  depth,
  start,
  end,
  is_dir: isDir,
  category: isDir ? null : 'video',
  ...extra,
});

// Focus 1. Ring 1: dir 10 (0..π), file 11 (π..1.5π), other (1.5π..2π).
// Ring 2: files 20 and 21 inside dir 10.
const arcs: SunburstArc[] = [
  arc(10, 1, 1, 0, Math.PI),
  arc(11, 1, 1, Math.PI, 1.5 * Math.PI, false),
  arc(OTHER_ARC_ID, 1, 1, 1.5 * Math.PI, TAU, false, { name: '', category: null }),
  arc(20, 10, 2, 0, Math.PI / 4, false),
  arc(21, 10, 2, Math.PI / 4, Math.PI / 2, false),
];
const root = { id: 1, name: 'root', size: 4000 };

// 400x400 canvas, dpr 1: centre (200, 200).
const layout = () => buildSunburstLayout(arcs, root, 1, 400, 400);

describe('sunburst geometry', () => {
  it('fits the rings into the canvas with a centre disc', () => {
    const g = sunburstGeometry(400, 400, 4, 1);
    expect(g.cx).toBe(200);
    expect(g.cy).toBe(200);
    expect(g.hole).toBeGreaterThanOrEqual(34);
    expect(g.hole + 4 * g.ring).toBeLessThanOrEqual(200);
    expect(g.ring).toBeGreaterThan(0);
  });

  it('caps the ring thickness and survives degenerate canvases', () => {
    expect(sunburstGeometry(2000, 2000, 1, 1).ring).toBeLessThanOrEqual(96);
    const tiny = sunburstGeometry(4, 4, 3, 1);
    expect(Number.isFinite(tiny.ring) && tiny.ring >= 0).toBe(true);
    expect(sunburstGeometry(0, 0, 0, 1).ring).toBe(0);
  });

  it('scales with the device pixel ratio', () => {
    const a = sunburstGeometry(400, 400, 3, 1);
    const b = sunburstGeometry(800, 800, 3, 2);
    expect(b.hole).toBeCloseTo(2 * a.hole, 5);
    expect(b.ring).toBeCloseTo(2 * a.ring, 5);
  });
});

describe('sunburst layout', () => {
  it('derives rings, parents, ring ranges and palette slots', () => {
    const l = layout();
    expect(l.rings).toBe(2);
    expect(l.ringStart).toEqual([0, 3, 5]);
    expect(Array.from(l.parentIndex)).toEqual([-1, -1, -1, 0, 0]);
    expect(Array.from(l.slot)).toEqual([0, 1, 2, 0, 0]);
    expect(l.index.get(21)).toBe(4);
    expect(l.index.has(OTHER_ARC_ID)).toBe(false);
    expect(isNodeArc(l, 2)).toBe(false);
    expect(isNodeArc(l, 3)).toBe(true);
  });

  it('repairs arcs delivered out of order and handles an empty result', () => {
    const l = buildSunburstLayout([...arcs].reverse(), root, 1, 400, 400);
    expect(l.arcs.map((a) => a.id)).toEqual([10, 11, OTHER_ARC_ID, 20, 21]);
    const none = buildSunburstLayout([], root, 1, 400, 400);
    expect(none.rings).toBe(0);
    expect(sunburstHitTest(none, 200, 100)).toEqual({ zone: 'none' });
  });

  it('builds the breadcrumb path from the arc up to the focus', () => {
    const l = layout();
    const focus = [{ id: 1, name: 'root' }];
    expect(sunburstCrumbsTo(l, 4, focus).map((c) => c.id)).toEqual([1, 10, 21]);
    expect(sunburstCrumbsTo(l, 1, focus).map((c) => c.id)).toEqual([1, 11]);
  });
});

describe('normalizeAngle', () => {
  it('wraps any angle into [0, TAU)', () => {
    expect(normalizeAngle(-Math.PI / 2)).toBeCloseTo(1.5 * Math.PI, 9);
    expect(normalizeAngle(TAU)).toBeCloseTo(0, 9);
    expect(normalizeAngle(2.5 * TAU)).toBeCloseTo(Math.PI, 9);
    expect(normalizeAngle(Number.NaN)).toBe(0);
    expect(normalizeAngle(Number.POSITIVE_INFINITY)).toBe(0);
  });
});

describe('sunburst hit test', () => {
  const l = layout();
  const { cx, cy, hole, ring } = l;
  /** Point at clockwise angle `a` from 12 o'clock and radius `r`. */
  const at = (a: number, r: number): [number, number] => [cx + r * Math.sin(a), cy - r * Math.cos(a)];

  it('finds the centre disc', () => {
    expect(sunburstHitTest(l, cx, cy)).toEqual({ zone: 'center' });
    expect(sunburstHitTest(l, ...at(1, hole - 1))).toEqual({ zone: 'center' });
  });

  it('maps the radius to a ring and the angle to an arc, clockwise from 12 o clock', () => {
    expect(sunburstHitTest(l, ...at(0.5, hole + ring / 2))).toEqual({ zone: 'arc', index: 0 });
    // Three o'clock is a quarter turn: still inside the first directory.
    expect(sunburstHitTest(l, ...at(Math.PI / 2, hole + ring / 2))).toEqual({ zone: 'arc', index: 0 });
    // Six o'clock to nine o'clock belongs to the file.
    expect(sunburstHitTest(l, ...at(1.25 * Math.PI, hole + ring / 2))).toEqual({ zone: 'arc', index: 1 });
    expect(sunburstHitTest(l, ...at(1.75 * Math.PI, hole + ring / 2))).toEqual({ zone: 'arc', index: 2 });
    expect(sunburstHitTest(l, ...at(0.1, hole + 1.5 * ring))).toEqual({ zone: 'arc', index: 3 });
    expect(sunburstHitTest(l, ...at(1.0, hole + 1.5 * ring))).toEqual({ zone: 'arc', index: 4 });
  });

  it('reports nothing in gaps, outside the rings and for non-finite points', () => {
    // Second ring only covers the first quarter turn.
    expect(sunburstHitTest(l, ...at(Math.PI, hole + 1.5 * ring))).toEqual({ zone: 'none' });
    expect(sunburstHitTest(l, ...at(0.2, hole + 2.5 * ring))).toEqual({ zone: 'none' });
    expect(sunburstHitTest(l, Number.NaN, 3)).toEqual({ zone: 'none' });
  });

  it('treats the seam at 12 o clock consistently', () => {
    expect(sunburstHitTest(l, cx, cy - hole - 2)).toEqual({ zone: 'arc', index: 0 });
    expect(sunburstHitTest(l, cx - 0.01, cy - hole - 2)).toEqual({ zone: 'arc', index: 2 });
  });

  it('gives each arc its ring radii', () => {
    expect(arcRadii(l, 3)).toEqual({ r0: hole + ring, r1: hole + 2 * ring });
  });
});

describe('arcColor', () => {
  const dir = arcs[0];
  const file = arcs[3];
  it('uses the treemap palette by first-ring slot and depth in folder mode', () => {
    expect(arcColor('dark', 'depth', file, 3)).toBe(treemapColor('dark', 3, 2, 'file'));
    expect(arcColor('light', 'depth', dir, 1)).toBe(treemapColor('light', 1, 1, 'header'));
    expect(arcColor('dark', 'depth', file, 3)).not.toBe(arcColor('dark', 'depth', dir, 3));
  });

  it('colours files by category and keeps folders neutral in file type mode', () => {
    expect(arcColor('light', 'category', file, 3)).toBe(categoryColor('light', 'video', 'file'));
    expect(arcColor('light', 'category', dir, 3)).toBe(categoryColor('light', null, 'header'));
    expect(arcColor('light', 'category', dir, 0)).toBe(arcColor('light', 'category', dir, 5));
  });

  it('draws the merged arc neutral in every mode', () => {
    const other = arcs[2];
    expect(arcColor('dark', 'depth', other, 2)).toBe(categoryColor('dark', null, 'file'));
    expect(arcColor('dark', 'category', other, 2)).toBe(categoryColor('dark', null, 'file'));
  });
});

describe('labelRoom', () => {
  it('prefers the direction with more room', () => {
    const wideArc = labelRoom(300, 40, 100, 13, 30);
    expect(wideArc?.mode).toBe('tangent');
    const thinArc = labelRoom(18, 90, 100, 13, 30);
    expect(thinArc?.mode).toBe('radial');
    expect(thinArc?.room).toBeGreaterThan(70);
  });

  it('refuses arcs that cannot hold a readable label', () => {
    expect(labelRoom(8, 90, 100, 13, 30)).toBeNull();
    expect(labelRoom(300, 10, 100, 13, 30)).toBeNull();
    expect(labelRoom(20, 20, 100, 13, 30)).toBeNull();
  });

  it('limits tangent text so the straight chord stays inside a curved ring', () => {
    const near = labelRoom(2000, 30, 40, 13, 10);
    expect(near?.room).toBeLessThan(2000 * 0.9);
    expect(near?.room).toBeCloseTo(Math.sqrt(0.3 * 30 * 8 * 40), 5);
  });
});

/** A canvas context that records calls and measures text as 6 px per character. */
function stubContext() {
  const calls: string[] = [];
  const ctx = new Proxy(
    {},
    {
      get: (_t, prop: string) => {
        if (prop === 'measureText') return (t: string) => ({ width: t.length * 6 });
        return (...args: unknown[]) => calls.push(`${prop}(${args.length})`);
      },
      set: () => true,
    },
  ) as unknown as CanvasRenderingContext2D;
  return { ctx, calls };
}

class FakePath2D {
  ops = 0;
  moveTo() { this.ops += 1; }
  arc() { this.ops += 1; }
  closePath() { this.ops += 1; }
}

describe('paintSunburst', () => {
  const colors = { background: '#222222', centerFill: '#333333', ink: '#ffffff', inkSub: '#cccccc' };

  it('batches arcs of the same colour into one fill and one stroke', () => {
    vi.stubGlobal('Path2D', FakePath2D);
    try {
      const many: SunburstArc[] = [];
      for (let i = 0; i < 2000; i += 1) {
        many.push(arc(100 + i, 1, 1, (i * TAU) / 2000, ((i + 1) * TAU) / 2000, false));
      }
      const l = buildSunburstLayout(many, root, 1, 800, 800);
      const { ctx, calls } = stubContext();
      paintSunburst(ctx, l, 'dark', 'category', colors, null);
      // All files are "video": a single colour group.
      expect(calls.filter((c) => c.startsWith('fill(1)')).length).toBe(1);
      expect(calls.filter((c) => c.startsWith('stroke(1)')).length).toBe(1);
      // A second paint with the same theme and mode reuses the cached paths.
      const again = stubContext();
      paintSunburst(again.ctx, l, 'dark', 'category', colors, null);
      expect(again.calls.length).toBeLessThanOrEqual(calls.length);
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('paints and marks changes without throwing', () => {
    vi.stubGlobal('Path2D', FakePath2D);
    try {
      const changed = arcs.map((a) => ({
        ...a,
        diff: a.id === 20 ? { status: 'added' as const, base: null, delta: 5e9, removed: null } : null,
      }));
      const l = buildSunburstLayout(changed, root, 2, 800, 800);
      const { ctx } = stubContext();
      paintSunburst(ctx, l, 'light', 'depth', colors, { grow: '#9a3412', shrink: '#17663a', halo: '#ffffff' });
      paintSunburstHighlights(ctx, l, 3, true, 4, [0, 3], {
        accent: '#ffffff',
        hoverFill: '#ffffff',
        hoverLine: '#ffffff',
        selectedInner: '#000000',
        queued: '#ff0000',
      });
    } finally {
      vi.unstubAllGlobals();
    }
  });

  it('builds and hit-tests a 20 000 arc layout quickly', () => {
    const many: SunburstArc[] = [];
    for (let i = 0; i < 20_000; i += 1) {
      many.push(arc(100 + i, 1, 1, (i * TAU) / 20_000, ((i + 1) * TAU) / 20_000, false));
    }
    const t0 = performance.now();
    const l = buildSunburstLayout(many, root, 1, 800, 800);
    for (let i = 0; i < 1000; i += 1) sunburstHitTest(l, 400 + (i % 50), 100 + (i % 7));
    expect(performance.now() - t0).toBeLessThan(500);
    const hit = sunburstHitTest(l, 400, 400 - l.hole - 2);
    expect(hit.zone).toBe('arc');
  });
});
