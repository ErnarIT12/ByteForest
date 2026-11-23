/** Keyboard navigation over treemap rectangles. Pure geometry, no store access. */
import type { Rect } from './api';

export type Direction = 'left' | 'right' | 'up' | 'down';

/** Weight of the off-axis distance relative to the distance along the arrow axis. */
const CROSS_AXIS_WEIGHT = 2;

/** Index of the largest direct child of `rects[parentIndex]`, or -1 when it has none. */
export function largestChild(rects: Rect[], parentIndex = 0): number {
  const parent = rects[parentIndex];
  if (!parent) return -1;
  let best = -1;
  rects.forEach((r, i) => {
    if (i !== parentIndex && r.parent === parent.id && (best < 0 || r.size > rects[best].size)) {
      best = i;
    }
  });
  return best;
}

/**
 * Index of the sibling of `rects[from]` (same parent) nearest in `dir`, or -1 when there is
 * none. A candidate must lie beyond the current centre on the arrow axis; closeness is the
 * centre distance, with the off-axis offset weighted more so a move prefers the arrow's row
 * or column.
 */
export function nearestSibling(rects: Rect[], from: number, dir: Direction): number {
  const cur = rects[from];
  if (!cur || from <= 0) return -1;
  const cx = cur.x + cur.w / 2;
  const cy = cur.y + cur.h / 2;
  let best = -1;
  let bestScore = Infinity;
  rects.forEach((r, i) => {
    if (i === from || i === 0 || r.parent !== cur.parent) return;
    const dx = r.x + r.w / 2 - cx;
    const dy = r.y + r.h / 2 - cy;
    const along = dir === 'left' ? -dx : dir === 'right' ? dx : dir === 'up' ? -dy : dy;
    if (along <= 0) return;
    const across = dir === 'left' || dir === 'right' ? Math.abs(dy) : Math.abs(dx);
    const score = along + CROSS_AXIS_WEIGHT * across;
    if (score < bestScore) {
      bestScore = score;
      best = i;
    }
  });
  return best;
}
