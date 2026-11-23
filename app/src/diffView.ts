/** Presentation of a node's snapshot comparison result. */
import type { NodeDiff } from './api';
import { formatDelta } from './format';
import { strings } from './strings';

export type DeltaTone = 'added' | 'grown' | 'shrunk';

export interface DeltaCell {
  /** Text shown in the cell; the sign or the "New" badge carries the meaning, not colour. */
  text: string;
  tone: DeltaTone;
}

/** What the delta column shows for a node; null (nothing) when it did not change. */
export function deltaCell(diff: NodeDiff | null | undefined): DeltaCell | null {
  if (!diff) return null;
  switch (diff.status) {
    case 'added':
      return { text: strings.diffNew, tone: 'added' };
    case 'grown':
      return { text: formatDelta(diff.delta), tone: 'grown' };
    case 'shrunk':
      return { text: formatDelta(diff.delta), tone: 'shrunk' };
    default:
      return null;
  }
}
