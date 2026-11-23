/**
 * Map highlights for the snapshot comparison, shared by the treemap and the sunburst.
 * Changes are shown with an outline whose pattern differs per kind (solid, dashed, dotted)
 * and, where space permits, a small label, so colour is never the only cue.
 */
import type { NodeDiff } from './api';
import { formatDelta } from './format';
import { strings } from './strings';

export type DiffTone = 'added' | 'grown' | 'shrunk';

/** Colours of the marks; `halo` separates a mark from any fill (read from `--tm-sel-inner`). */
export interface DiffColors {
  grow: string;
  shrink: string;
  halo: string;
}

/** The kind of change worth marking, or null when nothing changed (or not comparing). */
export function diffTone(diff: NodeDiff | null | undefined): DiffTone | null {
  if (!diff) return null;
  return diff.status === 'unchanged' ? null : diff.status;
}

/** Texts for a changed item, longest first; the first one that fits is shown. */
export function diffLabels(diff: NodeDiff | null | undefined): string[] {
  const tone = diffTone(diff);
  if (!diff || !tone) return [];
  if (tone === 'added') return [`${strings.diffNew} ${formatDelta(diff.delta)}`, strings.diffNew];
  return [formatDelta(diff.delta)];
}

/** Mark colour of a kind of change: growth and new items share the "grow" colour. */
export function toneColor(tone: DiffTone, colors: DiffColors): string {
  return tone === 'shrunk' ? colors.shrink : colors.grow;
}

export interface MarkStyle {
  /** Line width in device pixels. */
  width: number;
  /** Dash pattern in device pixels; empty for a solid line. */
  dash: number[];
  cap: 'butt' | 'round';
}

/** Outline pattern: grown = solid, shrunk = dashed, new = dotted. */
export function markStyle(tone: DiffTone, dpr: number, strong = true): MarkStyle {
  const width = Math.max(1, Math.round((strong ? 2 : 1) * dpr));
  switch (tone) {
    case 'grown':
      return { width, dash: [], cap: 'butt' };
    case 'shrunk':
      return { width, dash: [8 * dpr, 4 * dpr], cap: 'butt' };
    case 'added':
      return { width, dash: [0.01, 3.5 * dpr], cap: 'round' };
  }
}

/** 0 = unmarked, 1 = ancestor of a change (thin mark), 2 = the change itself (strong mark). */
export type MarkLevel = 0 | 1 | 2;

/**
 * Mark level per item. `parents[i]` is the index of item i's parent (-1 for none), parents
 * come before their children. A changed item that has a changed descendant is only an
 * ancestor mark, so a deep change does not outline every folder above it heavily.
 */
export function computeMarkLevels(
  parents: ArrayLike<number>,
  tones: ArrayLike<DiffTone | null>,
): Uint8Array {
  const n = tones.length;
  const levels = new Uint8Array(n);
  const hasChanged = new Uint8Array(n);
  for (let i = n - 1; i >= 0; i -= 1) {
    const changed = tones[i] !== null;
    if (changed) levels[i] = hasChanged[i] ? 1 : 2;
    const p = parents[i];
    if ((changed || hasChanged[i]) && p >= 0 && p < n) hasChanged[p] = 1;
  }
  return levels;
}

/** Most arcs / rectangles that get an outline per paint, so a huge comparison stays fast. */
export const MAX_MARKED = 4000;

const PILL_FONT = '600 11px "Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif';

/** Applies the line style of a mark to `ctx` (colour is set by the caller). */
export function applyMarkStyle(ctx: CanvasRenderingContext2D, style: MarkStyle): void {
  ctx.lineWidth = style.width;
  ctx.lineCap = style.cap;
  ctx.setLineDash(style.dash);
}

/**
 * Draws a small filled pill with the first of `labels` that fits `maxW`, its bottom-right
 * corner at (`right`, `bottom`). Returns whether anything was drawn.
 */
export function paintDeltaPill(
  ctx: CanvasRenderingContext2D,
  labels: readonly string[],
  fill: string,
  textColor: string,
  right: number,
  bottom: number,
  maxW: number,
  dpr: number,
): boolean {
  const padX = 4 * dpr;
  const height = 14 * dpr;
  ctx.font = PILL_FONT.replace('11px', `${Math.round(11 * dpr)}px`);
  for (const text of labels) {
    const width = ctx.measureText(text).width + 2 * padX;
    if (width > maxW) continue;
    ctx.setLineDash([]);
    ctx.fillStyle = fill;
    ctx.fillRect(right - width, bottom - height, width, height);
    ctx.fillStyle = textColor;
    ctx.textBaseline = 'middle';
    ctx.textAlign = 'left';
    ctx.fillText(text, right - width + padX, bottom - height / 2);
    return true;
  }
  return false;
}
