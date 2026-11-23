/**
 * Treemap geometry helpers and Canvas 2D painting. Everything works in device pixels:
 * rectangles are requested at `css size * devicePixelRatio` and drawn 1:1.
 */
import type { FileCategory, Rect } from './api';
import { contrastRatio } from './contrast';
import { formatBytes } from './format';
import type { ColorMode } from './hooks/useTreemapColorMode';
import type { Crumb } from './store';

export type Theme = 'light' | 'dark';

/** Name strip height of large directories, in CSS pixels. */
export const HEADER_CSS_PX = 16;
/** Smallest rectangle (CSS pixels) that gets a text label. */
const LABEL_MIN_W = 60;
const LABEL_MIN_H = 18;
/** A second line with the size needs this much height (CSS pixels). */
const SIZE_LINE_MIN_H = 34;

const FONT_STACK = '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif';

/** Muted blue, jade, amber and violet groups distinguish neighboring directory branches. */
export const HUES = [211, 165, 34, 254, 187, 355, 83, 233];

/** Display order of the file categories (same as `bf_core::FileCategory::ALL`). */
export const CATEGORIES: readonly FileCategory[] = [
  'video',
  'archive',
  'diskimage',
  'codebuild',
  'media',
  'system',
  'other',
];

/** Fixed hue per file category in the "File types" colour mode. */
export const CATEGORY_HUES: Record<FileCategory, number> = {
  video: 8,
  archive: 45,
  diskimage: 270,
  codebuild: 145,
  media: 205,
  system: 320,
  other: 220,
};

/** Saturation multiplier for neutral colours (folders in category mode, the "Other" files). */
const NEUTRAL_SATURATION = 0.18;

export interface TreemapLayout {
  rects: Rect[];
  /** Node id → index into `rects`. */
  index: Map<number, number>;
  /** Palette slot per rect: the slot of its top-level ancestor (-1 for the focused root). */
  slot: Int16Array;
  dpr: number;
  /** Header strip height in device pixels. */
  header: number;
  width: number;
  height: number;
}

/** Mirrors `bf_core::header_fits`: whether a non-root directory got a name strip. */
export function headerFits(w: number, h: number, header: number): boolean {
  return header > 0 && w >= 3 * header && h >= 3 * header;
}

export function buildLayout(
  rects: Rect[],
  dpr: number,
  header: number,
  width: number,
  height: number,
): TreemapLayout {
  const index = new Map<number, number>();
  const slot = new Int16Array(rects.length).fill(-1);
  let next = 0;
  rects.forEach((r, i) => {
    index.set(r.id, i);
    if (r.depth === 1) {
      slot[i] = next % HUES.length;
      next += 1;
    } else if (r.depth > 1 && r.parent !== null) {
      // Breadth-first order: the parent is already indexed.
      slot[i] = slot[index.get(r.parent) ?? 0];
    }
  });
  return { rects, index, slot, dpr, header, width, height };
}

/** Index of the deepest rectangle under device pixel (x, y), or -1. */
export function hitTest(layout: TreemapLayout, x: number, y: number): number {
  const { rects } = layout;
  // Deeper rectangles come later and siblings never overlap: the last hit is the deepest.
  for (let i = rects.length - 1; i >= 0; i -= 1) {
    const r = rects[i];
    if (x >= r.x && y >= r.y && x < r.x + r.w && y < r.y + r.h) return i;
  }
  return -1;
}

/** Focus path extended down to rect `i`: `focusPath` followed by the ancestors of `i`. */
export function crumbsTo(layout: TreemapLayout, i: number, focusPath: Crumb[]): Crumb[] {
  const chain: Crumb[] = [];
  let cur = i;
  while (cur > 0) {
    const r = layout.rects[cur];
    chain.push({ id: r.id, name: r.name });
    cur = r.parent === null ? -1 : (layout.index.get(r.parent) ?? -1);
  }
  return [...focusPath, ...chain.reverse()];
}

export type Role = 'file' | 'dirFill' | 'dirStroke' | 'header';

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** Fill colour (`#rrggbb`) of a rectangle role for a palette slot and depth. */
export function treemapColor(theme: Theme, slot: number, depth: number, role: Role): string {
  const [h, s, l] = hsl(theme, HUES[Math.max(0, slot)], depth, role);
  return hslToHex(h, s, l);
}

/**
 * Colour of a rectangle in the "File types" mode. Files get their category's hue (the same
 * for every depth); folders are neutral so only files carry colour.
 */
export function categoryColor(theme: Theme, category: FileCategory | null, role: Role): string {
  const neutral = role !== 'file' || category === null || category === 'other';
  const hue = CATEGORY_HUES[neutral ? 'other' : category];
  const [h, s, l] = hsl(theme, hue, 1, role);
  return hslToHex(h, neutral ? s * NEUTRAL_SATURATION : s, l);
}

function hsl(theme: Theme, h: number, depth: number, role: Role): [number, number, number] {
  const d = Math.max(0, depth - 1);
  if (theme === 'light') {
    switch (role) {
      case 'file':
        return [h, 37, clamp(76 - 4 * d, 62, 82)];
      case 'dirFill':
        return [h, 32, clamp(94 - 4 * d, 72, 96)];
      case 'dirStroke':
        return [h, 30, clamp(60 - 5 * d, 38, 66)];
      case 'header':
        return [h, 38, clamp(84 - 4 * d, 68, 88)];
    }
  }
  switch (role) {
    case 'file':
      return [h, 30, clamp(38 - 2 * d, 27, 38)];
    case 'dirFill':
      return [h, 26, clamp(14 + 2 * d, 12, 22)];
    case 'dirStroke':
      return [h, 28, clamp(30 + 3 * d, 26, 44)];
    case 'header':
      return [h, 23, clamp(23 + 2 * d, 23, 33)];
  }
}

function hslToHex(h: number, sPct: number, lPct: number): string {
  const sat = sPct / 100;
  const light = lPct / 100;
  const a = sat * Math.min(light, 1 - light);
  const channel = (n: number) => {
    const k = (n + h / 30) % 12;
    const v = light - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
    return Math.round(v * 255)
      .toString(16)
      .padStart(2, '0');
  };
  return `#${channel(0)}${channel(8)}${channel(4)}`;
}

const LIGHT_LABEL = { main: '#ffffff', sub: '#e3e5e9' };
const DARK_LABEL = { main: '#08090a', sub: '#26292d' };

/** Label colours for text drawn on `fill`: whichever of light/dark text contrasts more. */
export function labelColors(fill: string): { main: string; sub: string } {
  return contrastRatio(fill, LIGHT_LABEL.main) >= contrastRatio(fill, DARK_LABEL.main)
    ? LIGHT_LABEL
    : DARK_LABEL;
}

/** Longest prefix of `text` (with an ellipsis) that fits `maxW`, or "" if nothing fits. */
function fitText(ctx: CanvasRenderingContext2D, text: string, maxW: number): string {
  if (ctx.measureText(text).width <= maxW) return text;
  let lo = 0;
  let hi = text.length;
  while (lo < hi) {
    const mid = Math.ceil((lo + hi) / 2);
    if (ctx.measureText(`${text.slice(0, mid)}…`).width <= maxW) lo = mid;
    else hi = mid - 1;
  }
  return lo === 0 ? '' : `${text.slice(0, lo)}…`;
}

/** Paints all rectangles in one pass, in array order (parents before children). */
export function paintTreemap(
  ctx: CanvasRenderingContext2D,
  layout: TreemapLayout,
  theme: Theme,
  background: string,
  mode: ColorMode = 'depth',
): void {
  const { rects, slot, dpr, header } = layout;
  const cache = new Map<string, string>();
  const paint = (s: number, depth: number, role: Role, category: FileCategory | null = null) => {
    // In category mode only the category and role matter, not the slot or depth.
    const key =
      mode === 'category' ? `c|${category ?? '-'}|${role}` : `d|${s}|${depth}|${role}`;
    let c = cache.get(key);
    if (c === undefined) {
      c =
        mode === 'category'
          ? categoryColor(theme, category, role)
          : treemapColor(theme, s, depth, role);
      cache.set(key, c);
    }
    return c;
  };
  const labelW = LABEL_MIN_W * dpr;
  const labelH = LABEL_MIN_H * dpr;
  const pad = Math.round(4 * dpr);
  const font = `${Math.round(12 * dpr)}px ${FONT_STACK}`;
  const headerFont = `600 ${Math.round(11.5 * dpr)}px ${FONT_STACK}`;

  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.fillStyle = background;
  ctx.fillRect(0, 0, layout.width, layout.height);
  ctx.textBaseline = 'middle';
  ctx.lineWidth = 1;

  for (let i = 1; i < rects.length; i += 1) {
    const r = rects[i];
    const s = slot[i];
    if (r.is_dir) {
      ctx.fillStyle = paint(s, r.depth, 'dirFill');
      ctx.fillRect(r.x, r.y, r.w, r.h);
      if (r.w > 2 && r.h > 2) {
        ctx.strokeStyle = paint(s, r.depth, 'dirStroke');
        ctx.strokeRect(r.x + 0.5, r.y + 0.5, r.w - 1, r.h - 1);
      }
      if (headerFits(r.w, r.h, header)) {
        const headerFill = paint(s, r.depth, 'header');
        ctx.fillStyle = headerFill;
        ctx.fillRect(r.x + 1, r.y + 1, r.w - 2, header);
        if (r.w >= labelW) {
          ctx.font = headerFont;
          const label = fitText(ctx, r.name, r.w - 2 * pad);
          if (label) {
            ctx.fillStyle = labelColors(headerFill).main;
            ctx.fillText(label, r.x + pad, r.y + 1 + header / 2);
          }
        }
      }
      continue;
    }
    // Files: filled tiles with a 1 px gap so neighbours stay distinguishable.
    const gap = r.w > 3 && r.h > 3 ? 1 : 0;
    const fileFill = paint(s, r.depth, 'file', r.category);
    const text = labelColors(fileFill);
    ctx.fillStyle = fileFill;
    ctx.fillRect(r.x, r.y, r.w - gap, r.h - gap);
    if (r.w >= labelW && r.h >= labelH) {
      ctx.font = font;
      const label = fitText(ctx, r.name, r.w - 2 * pad);
      if (!label) continue;
      const twoLines = r.h >= SIZE_LINE_MIN_H * dpr;
      const lineH = Math.round(15 * dpr);
      const top = twoLines ? r.y + pad + lineH / 2 : r.y + r.h / 2;
      ctx.fillStyle = text.main;
      ctx.fillText(label, r.x + pad, top);
      if (twoLines) {
        ctx.fillStyle = text.sub;
        ctx.fillText(formatBytes(r.size), r.x + pad, top + lineH);
      }
    }
  }
}

/** Overlay colours, read from the theme's CSS tokens. */
export interface HighlightColors {
  accent: string;
  hoverFill: string;
  hoverLine: string;
  selectedInner: string;
}

/** Paints the hover and selection outlines on the overlay canvas. */
export function paintHighlights(
  ctx: CanvasRenderingContext2D,
  layout: TreemapLayout,
  hover: number,
  selected: number,
  colors: HighlightColors,
): void {
  const { rects, dpr } = layout;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.clearRect(0, 0, layout.width, layout.height);
  const outline = (i: number, width: number, style: string, inset: number) => {
    const r = rects[i];
    const o = inset + width / 2;
    if (r.w <= 2 * o || r.h <= 2 * o) return;
    ctx.lineWidth = width;
    ctx.strokeStyle = style;
    ctx.strokeRect(r.x + o, r.y + o, r.w - 2 * o, r.h - 2 * o);
  };
  if (hover > 0 && hover !== selected) {
    const r = rects[hover];
    ctx.fillStyle = colors.hoverFill;
    ctx.fillRect(r.x, r.y, r.w, r.h);
    outline(hover, Math.max(1, Math.round(1.5 * dpr)), colors.hoverLine, 0);
  }
  if (selected > 0) {
    const w = Math.max(2, Math.round(2 * dpr));
    outline(selected, w, colors.accent, 0);
    outline(selected, Math.max(1, Math.round(dpr)), colors.selectedInner, w);
  }
}
