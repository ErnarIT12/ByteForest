import { describe, expect, it } from 'vitest';
import { OTHER_ARC_ID } from './api';
import { largestChildArc, parentArc, siblingArc } from './sunburstNav';

type A = { id: number; depth: number };

// Ring 1: 10, 11, other. Ring 2: 20, 21 under 10; 22, other under 11.
const arcs = [
  { id: 10, depth: 1 },
  { id: 11, depth: 1 },
  { id: OTHER_ARC_ID, depth: 1 },
  { id: 20, depth: 2 },
  { id: 21, depth: 2 },
  { id: 22, depth: 2 },
  { id: OTHER_ARC_ID, depth: 2 },
] as A[] as never;
const parentIndex = Int32Array.from([-1, -1, -1, 0, 0, 1, 1]);
const layout = { arcs, parentIndex } as never;

describe('sunburst keyboard navigation', () => {
  it('moves to the next and previous sibling within the same parent', () => {
    expect(siblingArc(layout, 3, 1)).toBe(4);
    expect(siblingArc(layout, 4, -1)).toBe(3);
    expect(siblingArc(layout, 0, 1)).toBe(1);
  });

  it('stops at the ends of the siblings and never enters the other arc or a neighbour parent', () => {
    expect(siblingArc(layout, 3, -1)).toBe(-1);
    expect(siblingArc(layout, 4, 1)).toBe(-1);
    // 11 is followed only by the merged arc.
    expect(siblingArc(layout, 1, 1)).toBe(-1);
    expect(siblingArc(layout, 5, 1)).toBe(-1);
    expect(siblingArc(layout, 99, 1)).toBe(-1);
  });

  it('goes up to the parent arc, or nowhere from the first ring', () => {
    expect(parentArc(layout, 4)).toBe(0);
    expect(parentArc(layout, 5)).toBe(1);
    expect(parentArc(layout, 0)).toBe(-1);
    expect(parentArc(layout, -1)).toBe(-1);
  });

  it('goes down to the largest child, which is the first real one', () => {
    expect(largestChildArc(layout, 0)).toBe(3);
    expect(largestChildArc(layout, 1)).toBe(5);
    // The first ring is the child list of the focused folder.
    expect(largestChildArc(layout, -1)).toBe(0);
  });

  it('finds no child below a file or below the outermost ring', () => {
    expect(largestChildArc(layout, 3)).toBe(-1);
    expect(largestChildArc(layout, 2)).toBe(-1);
    expect(largestChildArc({ arcs: [], parentIndex: new Int32Array(0) } as never, -1)).toBe(-1);
  });
});
