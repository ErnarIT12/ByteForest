/** Keyboard navigation over sunburst arcs. Pure index arithmetic, no store access. */
import { OTHER_ARC_ID } from './api';
import type { SunburstLayout } from './sunburstPaint';

type NavLayout = Pick<SunburstLayout, 'arcs' | 'parentIndex'>;

/** The arc's neighbour in the same ring and parent, skipping the merged "other" arc; -1 at the ends. */
export function siblingArc(layout: NavLayout, from: number, step: 1 | -1): number {
  const { arcs, parentIndex } = layout;
  if (from < 0 || from >= arcs.length) return -1;
  for (let i = from + step; i >= 0 && i < arcs.length; i += step) {
    if (arcs[i].depth !== arcs[from].depth || parentIndex[i] !== parentIndex[from]) return -1;
    if (arcs[i].id !== OTHER_ARC_ID) return i;
  }
  return -1;
}

/** Parent arc of `from`; -1 for the first ring (its parent is the focused folder). */
export function parentArc(layout: NavLayout, from: number): number {
  return from >= 0 && from < layout.arcs.length ? layout.parentIndex[from] : -1;
}

/** Largest child of `parent` (-1 means the focused folder, i.e. the first ring); -1 if none. */
export function largestChildArc(layout: NavLayout, parent: number): number {
  const { arcs, parentIndex } = layout;
  const depth = parent < 0 ? 1 : (arcs[parent]?.depth ?? 0) + 1;
  // Children are ordered by size, so the first real arc of the right parent is the largest.
  for (let i = 0; i < arcs.length; i += 1) {
    if (arcs[i].depth === depth && parentIndex[i] === parent && arcs[i].id !== OTHER_ARC_ID) return i;
    if (arcs[i].depth > depth) break;
  }
  return -1;
}
