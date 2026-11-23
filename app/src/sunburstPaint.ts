/**
 * Sunburst geometry, hit testing and Canvas 2D painting. Everything works in device pixels;
 * angles are radians clockwise from 12 o'clock, as delivered by the backend.
 *
 * Painting is batched: arcs sharing a colour go into one `Path2D`, cached per layout, theme
 * and colour mode, so a repaint is one fill and one stroke per colour (about a hundred calls
 * for 20 000 arcs) and hover/selection never touch the base layer.
 */
import { OTHER_ARC_ID, type FileCategory, type SunburstArc } from './api';
import {
  MAX_MARKED,
  applyMarkStyle,
  computeMarkLevels,
  diffLabels,
  diffTone,
  markStyle,
  toneColor,
  type DiffColors,
  type DiffTone,
} from './diffMarks';
import type { ColorMode } from './hooks/useTreemapColorMode';
import { formatBytes } from './format';
import type { Crumb } from './store';
import {
  HUES,
  categoryColor,
  hatchLines,
  labelColors,
  treemapColor,
  type Role,
  type Theme,
} from './treemapPaint';

const TAU = Math.PI * 2;
const FONT_STACK = '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif';

/** Widest a ring gets, CSS pixels: with few rings the chart is smaller than the panel. */
const MAX_RING_CSS = 96;
/** Smallest centre disc radius, CSS pixels. */
const MIN_HOLE_CSS = 34;

export interface SunburstRoot {
  id: number;
  name: string;
  size: number;
}

export interface SunburstLayout {
  /** Ring by ring, each ring ordered by start angle. */
  arcs: SunburstArc[];
  /** Node id → index into `arcs` (the merged "other" arcs are not indexed). */
  index: Map<number, number>;
  /** Palette slot per arc: the slot of its first-ring ancestor. */
  slot: Int16Array;
  /** Index of the parent arc, -1 for the first ring. */
  parentIndex: Int32Array;
  /** Ring r (1-based) is `arcs[ringStart[r - 1] .. ringStart[r])`. */
  ringStart: number[];
  rings: number;
  dpr: number;
  width: number;
  height: number;
  cx: number;
  cy: number;
  /** Radius of the centre disc and thickness of a ring, device pixels. */
  hole: number;
  ring: number;
  root: SunburstRoot;
}

export interface Geometry {
  cx: number;
  cy: number;
  hole: number;
  ring: number;
}

/** Centre, centre disc radius and ring thickness that fit `rings` rings into the canvas. */
export function sunburstGeometry(
  width: number,
  height: number,
  rings: number,
  dpr: number,
): Geometry {
  const cx = width / 2;
  const cy = height / 2;
  const radius = Math.max(0, Math.min(width, height) / 2 - 6 * dpr);
  const hole = Math.min(radius * 0.4, Math.max(MIN_HOLE_CSS * dpr, radius * 0.2));
  const ring = Math.min(
    MAX_RING_CSS * dpr,
    Math.max(0, radius - hole) / Math.max(1, rings),
  );
  return { cx, cy, hole, ring };
}

export function buildSunburstLayout(
  input: SunburstArc[],
  root: SunburstRoot,
  dpr: number,
  width: number,
  height: number,
): SunburstLayout {
  // The backend delivers ring by ring and by angle; repair the order if it ever does not.
  let arcs = input;
  for (let i = 1; i < arcs.length; i += 1) {
    const a = arcs[i - 1];
    const b = arcs[i];
    if (b.depth < a.depth || (b.depth === a.depth && b.start < a.start)) {
      arcs = [...input].sort((p, q) => p.depth - q.depth || p.start - q.start);
      break;
    }
  }
  const index = new Map<number, number>();
  const slot = new Int16Array(arcs.length).fill(-1);
  const parentIndex = new Int32Array(arcs.length).fill(-1);
  const ringStart: number[] = [0];
  let next = 0;
  let rings = 0;
  arcs.forEach((a, i) => {
    while (rings < a.depth) {
      ringStart.push(i);
      rings += 1;
    }
    ringStart[a.depth] = i + 1;
    if (a.id !== OTHER_ARC_ID) index.set(a.id, i);
    if (a.depth === 1) {
      slot[i] = next % HUES.length;
      next += 1;
    } else {
      const p = index.get(a.parent);
      if (p !== undefined) {
        parentIndex[i] = p;
        slot[i] = slot[p];
      }
    }
  });
  // Rings without arcs in the middle cannot occur (a ring needs its parent ring); fix up the
  // offsets anyway so every ring has a valid range.
  for (let r = 1; r < ringStart.length; r += 1) {
    if (ringStart[r] < ringStart[r - 1]) ringStart[r] = ringStart[r - 1];
  }
  const g = sunburstGeometry(width, height, rings, dpr);
  return { arcs, index, slot, parentIndex, ringStart, rings, dpr, width, height, ...g, root };
}

/** Angle in `[0, TAU)` for any finite angle. */
export function normalizeAngle(a: number): number {
  if (!Number.isFinite(a)) return 0;
  const r = a % TAU;
  return r < 0 ? r + TAU : r;
}

export type SunburstHit =
  | { zone: 'none' }
  | { zone: 'center' }
  | { zone: 'arc'; index: number };

/** What lies under device pixel (x, y): the centre disc, an arc, or nothing. */
export function sunburstHitTest(layout: SunburstLayout, x: number, y: number): SunburstHit {
  const dx = x - layout.cx;
  const dy = y - layout.cy;
  const r = Math.hypot(dx, dy);
  if (!Number.isFinite(r)) return { zone: 'none' };
  if (r < layout.hole) return { zone: 'center' };
  if (layout.ring <= 0) return { zone: 'none' };
  const depth = Math.floor((r - layout.hole) / layout.ring) + 1;
  if (depth < 1 || depth > layout.rings) return { zone: 'none' };
  // Clockwise from 12 o'clock.
  const angle = normalizeAngle(Math.atan2(dx, -dy));
  const { arcs, ringStart } = layout;
  let lo = ringStart[depth - 1];
  let hi = ringStart[depth] - 1;
  let found = -1;
  // Last arc of the ring starting at or before the angle.
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (arcs[mid].start <= angle) {
      found = mid;
      lo = mid + 1;
    } else {
      hi = mid - 1;
    }
  }
  if (found < 0 || angle >= arcs[found].end) return { zone: 'none' };
  return { zone: 'arc', index: found };
}

/** Focus path extended down to arc `i`: `focusPath` followed by the arc's ancestors. */
export function sunburstCrumbsTo(layout: SunburstLayout, i: number, focusPath: Crumb[]): Crumb[] {
  const chain: Crumb[] = [];
  let cur = i;
  while (cur >= 0) {
    const a = layout.arcs[cur];
    chain.push({ id: a.id, name: a.name });
    cur = layout.parentIndex[cur];
  }
  return [...focusPath, ...chain.reverse()];
}

/** Whether arc `i` stands for a real node (not the merged "other" arc). */
export function isNodeArc(layout: SunburstLayout, i: number): boolean {
  return i >= 0 && i < layout.arcs.length && layout.arcs[i].id !== OTHER_ARC_ID;
}

/** Radii of arc `i`, device pixels. */
export function arcRadii(layout: SunburstLayout, i: number): { r0: number; r1: number } {
  const d = layout.arcs[i].depth;
  return { r0: layout.hole + (d - 1) * layout.ring, r1: layout.hole + d * layout.ring };
}

// ---- Colours ----------------------------------------------------------------------------

/** Fill of an arc. Folders use the header shade and files the file shade of the treemap. */
export function arcColor(
  theme: Theme,
  mode: ColorMode,
  arc: Pick<SunburstArc, 'id' | 'is_dir' | 'category' | 'depth'>,
  slot: number,
): string {
  const role: Role = arc.is_dir ? 'header' : 'file';
  if (arc.id === OTHER_ARC_ID) return categoryColor(theme, null, 'file');
  if (mode === 'category') return categoryColor(theme, arc.category as FileCategory | null, role);
  return treemapColor(theme, slot, arc.depth, role);
}

// ---- Path building ----------------------------------------------------------------------

type PathLike = Pick<Path2D, 'arc' | 'closePath' | 'moveTo'>;

/** Adds the closed ring segment to `p`; angles are in the canvas frame (0 = 3 o'clock). */
export function traceArc(p: PathLike, cx: number, cy: number, r0: number, r1: number, a0: number, a1: number): void {
  p.moveTo(cx + r1 * Math.cos(a0), cy + r1 * Math.sin(a0));
  p.arc(cx, cy, r1, a0, a1, false);
  p.arc(cx, cy, r0, a1, a0, true);
  p.closePath();
}

/** Canvas-frame start and end angle of arc `i` (the sunburst starts at 12 o'clock). */
function frameAngles(a: SunburstArc): [number, number] {
  return [a.start - Math.PI / 2, a.end - Math.PI / 2];
}

interface ColorGroup {
  color: string;
  fill: Path2D;
  stroke: Path2D;
}

const prepared = new WeakMap<SunburstLayout, { key: string; groups: ColorGroup[] }>();

function groupsFor(layout: SunburstLayout, theme: Theme, mode: ColorMode): ColorGroup[] {
  const key = `${theme}|${mode}`;
  const hit = prepared.get(layout);
  if (hit && hit.key === key) return hit.groups;
  const byColor = new Map<string, ColorGroup>();
  const { arcs, slot, cx, cy, dpr } = layout;
  for (let i = 0; i < arcs.length; i += 1) {
    const a = arcs[i];
    if (!(a.end > a.start)) continue;
    const color = arcColor(theme, mode, a, slot[i]);
    let g = byColor.get(color);
    if (!g) {
      g = { color, fill: new Path2D(), stroke: new Path2D() };
      byColor.set(color, g);
    }
    const { r0, r1 } = arcRadii(layout, i);
    const [a0, a1] = frameAngles(a);
    traceArc(g.fill, cx, cy, r0, r1, a0, a1);
    // Separators only where they leave room for the fill.
    if ((a.end - a.start) * r0 >= 3 * dpr) traceArc(g.stroke, cx, cy, r0, r1, a0, a1);
  }
  const groups = [...byColor.values()];
  prepared.set(layout, { key, groups });
  return groups;
}

// ---- Labels -----------------------------------------------------------------------------

export type LabelMode = 'tangent' | 'radial';

/**
 * How a label fits an arc: along the arc ("tangent", text on a straight chord, so the chord
 * may not sag more than 30% of the ring) or along the radius. Returns the mode with the most
 * room and that room in pixels, or null when less than `minRoom` is available. All values are
 * in the same unit (device pixels).
 */
export function labelRoom(
  arcLength: number,
  ringThickness: number,
  midRadius: number,
  lineHeight: number,
  minRoom: number,
): { mode: LabelMode; room: number } | null {
  const pad = lineHeight * 0.25;
  const tangent =
    ringThickness >= lineHeight + 2 * pad && midRadius > 0
      ? Math.min(arcLength * 0.9, Math.sqrt(0.3 * ringThickness * 8 * midRadius))
      : 0;
  const radial = arcLength >= lineHeight + pad ? ringThickness - 2 * pad : 0;
  const best = tangent >= radial ? { mode: 'tangent' as const, room: tangent } : { mode: 'radial' as const, room: radial };
  return best.room >= minRoom ? best : null;
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

// ---- Painting ---------------------------------------------------------------------------

/** Colours the base layer needs besides the arc fills; read from the theme's CSS tokens. */
export interface SunburstColors {
  background: string;
  centerFill: string;
  ink: string;
  inkSub: string;
}

/** Paints the arcs, labels, centre disc and (while comparing) change marks. */
export function paintSunburst(
  ctx: CanvasRenderingContext2D,
  layout: SunburstLayout,
  theme: Theme,
  mode: ColorMode,
  colors: SunburstColors,
  diffColors: DiffColors | null,
): void {
  const { arcs, slot, cx, cy, dpr, hole, ring } = layout;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.setLineDash([]);
  ctx.fillStyle = colors.background;
  ctx.fillRect(0, 0, layout.width, layout.height);

  const groups = groupsFor(layout, theme, mode);
  ctx.strokeStyle = colors.background;
  ctx.lineWidth = Math.max(1, Math.round(dpr));
  ctx.lineJoin = 'round';
  for (const g of groups) {
    ctx.fillStyle = g.color;
    ctx.fill(g.fill);
    ctx.stroke(g.stroke);
  }

  // Labels: only arcs with room are measured, so this stays cheap for 20 000 arcs.
  const lineH = Math.round(13 * dpr);
  const minRoom = 34 * dpr;
  ctx.font = `${Math.round(11.5 * dpr)}px ${FONT_STACK}`;
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  const marks = diffColors ? markLevels(layout) : null;
  for (let i = 0; i < arcs.length; i += 1) {
    const a = arcs[i];
    const span = a.end - a.start;
    const { r0, r1 } = arcRadii(layout, i);
    const mid = (r0 + r1) / 2;
    const room = labelRoom(span * mid, ring, mid, lineH, minRoom);
    if (!room) continue;
    const fill = arcColor(theme, mode, a, slot[i]);
    let text = a.name;
    if (a.id === OTHER_ARC_ID) text = '…';
    else if (marks?.levels[i] === 2) {
      // Append the delta where the whole text fits.
      for (const extra of diffLabels(a.diff)) {
        const full = `${a.name}  ${extra}`;
        if (ctx.measureText(full).width <= room.room) {
          text = full;
          break;
        }
      }
    }
    const label = fitText(ctx, text, room.room);
    if (!label) continue;
    const m = (a.start + a.end) / 2 - Math.PI / 2;
    ctx.save();
    ctx.translate(cx + mid * Math.cos(m), cy + mid * Math.sin(m));
    if (room.mode === 'tangent') {
      const rot = m + Math.PI / 2;
      // Keep the text upright on the lower half.
      ctx.rotate(Math.sin(m) > 0 ? rot + Math.PI : rot);
    } else {
      ctx.rotate(Math.cos(m) < 0 ? m + Math.PI : m);
    }
    ctx.fillStyle = labelColors(fill).main;
    ctx.fillText(label, 0, 0);
    ctx.restore();
  }

  if (diffColors && marks) paintArcMarks(ctx, layout, marks, diffColors);

  // Centre disc: the focused folder.
  ctx.beginPath();
  ctx.arc(cx, cy, Math.max(0, hole - Math.max(1, dpr)), 0, TAU);
  ctx.fillStyle = colors.centerFill;
  ctx.fill();
  if (hole >= 20 * dpr) {
    const maxW = hole * 1.55;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.font = `600 ${Math.round(13 * dpr)}px ${FONT_STACK}`;
    const name = fitText(ctx, layout.root.name, maxW);
    ctx.fillStyle = colors.ink;
    ctx.fillText(name, cx, cy - 8 * dpr);
    ctx.font = `${Math.round(12 * dpr)}px ${FONT_STACK}`;
    ctx.fillStyle = colors.inkSub;
    ctx.fillText(fitText(ctx, formatBytes(layout.root.size), maxW), cx, cy + 9 * dpr);
  }
}

interface Marks {
  levels: Uint8Array;
  tones: (DiffTone | null)[];
}

function markLevels(layout: SunburstLayout): Marks | null {
  const tones = layout.arcs.map((a) => diffTone(a.diff));
  if (!tones.some((t) => t !== null)) return null;
  return { levels: computeMarkLevels(layout.parentIndex, tones), tones };
}

/** Outlines changed arcs inside their own edge: solid grown, dashed shrunk, dotted new. */
function paintArcMarks(
  ctx: CanvasRenderingContext2D,
  layout: SunburstLayout,
  marks: Marks,
  colors: DiffColors,
): void {
  const { arcs, cx, cy, dpr } = layout;
  const halo = Math.max(1, Math.round(dpr));
  let drawn = 0;
  ctx.save();
  for (const level of [1, 2]) {
    for (let i = 0; i < arcs.length && drawn < MAX_MARKED; i += 1) {
      const tone = marks.tones[i];
      if (!tone || marks.levels[i] !== level) continue;
      drawn += 1;
      const a = arcs[i];
      const { r0, r1 } = arcRadii(layout, i);
      const [a0, a1] = frameAngles(a);
      const color = toneColor(tone, colors);
      const path = new Path2D();
      traceArc(path, cx, cy, r0, r1, a0, a1);
      if ((a.end - a.start) * r0 < 5 * dpr) {
        // Too thin for an outline: the whole arc takes the mark colour.
        ctx.fillStyle = color;
        ctx.fill(path);
        continue;
      }
      const style = markStyle(tone, dpr, level === 2);
      ctx.save();
      ctx.clip(path);
      // Strokes straddle the edge; the clip keeps only the inner half, hence the doubling.
      if (level === 2) {
        ctx.setLineDash([]);
        ctx.lineWidth = 2 * (style.width + halo);
        ctx.strokeStyle = colors.halo;
        ctx.stroke(path);
      }
      applyMarkStyle(ctx, { ...style, width: 2 * style.width });
      ctx.strokeStyle = color;
      ctx.stroke(path);
      ctx.restore();
    }
  }
  ctx.restore();
}

/** Overlay colours, read from the theme's CSS tokens. */
export interface SunburstHighlightColors {
  accent: string;
  hoverFill: string;
  hoverLine: string;
  selectedInner: string;
  queued: string;
}

/** Paints hover, selection and Discard Pile markers on the overlay canvas. */
export function paintSunburstHighlights(
  ctx: CanvasRenderingContext2D,
  layout: SunburstLayout,
  hover: number,
  hoverCenter: boolean,
  selected: number,
  queued: readonly number[],
  colors: SunburstHighlightColors,
): void {
  const { arcs, cx, cy, dpr, hole } = layout;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.setLineDash([]);
  ctx.clearRect(0, 0, layout.width, layout.height);
  const pathOf = (i: number, inset = 0) => {
    const a = arcs[i];
    const { r0, r1 } = arcRadii(layout, i);
    const [a0, a1] = frameAngles(a);
    const p = new Path2D();
    const ro0 = r0 + inset;
    const ro1 = r1 - inset;
    if (ro1 <= ro0) return null;
    // A constant pixel inset at the middle radius, converted to an angle.
    const da = Math.min((a1 - a0) / 2 - 1e-6, inset / Math.max(1, (r0 + r1) / 2));
    traceArc(p, cx, cy, ro0, ro1, a0 + da, a1 - da);
    return p;
  };
  for (const i of queued) {
    const a = arcs[i];
    const p = pathOf(i);
    if (!p || (a.end - a.start) * arcRadii(layout, i).r0 < 3 * dpr) continue;
    const { r1 } = arcRadii(layout, i);
    ctx.save();
    ctx.clip(p);
    ctx.globalAlpha = 0.5;
    ctx.strokeStyle = colors.queued;
    ctx.lineWidth = Math.max(1, Math.round(dpr));
    ctx.beginPath();
    for (const [x1, y1, x2, y2] of hatchLines(cx - r1, cy - r1, 2 * r1, 2 * r1, 7 * dpr)) {
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
    }
    ctx.stroke();
    ctx.globalAlpha = 1;
    ctx.setLineDash([4 * dpr, 3 * dpr]);
    ctx.lineWidth = 2 * Math.max(1, Math.round(1.5 * dpr));
    ctx.stroke(p);
    ctx.restore();
  }
  if (hoverCenter) {
    ctx.beginPath();
    ctx.arc(cx, cy, hole, 0, TAU);
    ctx.fillStyle = colors.hoverFill;
    ctx.fill();
  }
  if (hover >= 0 && hover !== selected) {
    const p = pathOf(hover);
    if (p) {
      ctx.fillStyle = colors.hoverFill;
      ctx.fill(p);
      ctx.save();
      ctx.clip(p);
      ctx.strokeStyle = colors.hoverLine;
      ctx.lineWidth = 2 * Math.max(1, Math.round(1.5 * dpr));
      ctx.stroke(p);
      ctx.restore();
    }
  }
  if (selected >= 0) {
    const p = pathOf(selected);
    if (p) {
      const w = Math.max(2, Math.round(2 * dpr));
      ctx.save();
      ctx.clip(p);
      ctx.strokeStyle = colors.accent;
      ctx.lineWidth = 2 * w;
      ctx.stroke(p);
      ctx.restore();
      const inner = pathOf(selected, w);
      if (inner) {
        ctx.strokeStyle = colors.selectedInner;
        ctx.lineWidth = Math.max(1, Math.round(dpr));
        ctx.stroke(inner);
      }
    }
  }
}
