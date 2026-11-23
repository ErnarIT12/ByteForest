import { useEffect, useReducer } from 'react';
import { OTHER_ARC_ID, type Rect } from '../api';
import { diffTone } from '../diffMarks';
import { formatBytes, formatDelta, formatPercent } from '../format';
import { cachedPath, loadPath } from '../pathCache';
import { strings } from '../strings';

/** Hover time before the full path is requested. */
const PATH_DELAY_MS = 80;

/** What the tooltip describes: a treemap rectangle or a sunburst arc. */
export type TooltipSubject = Pick<Rect, 'id' | 'name' | 'size' | 'is_dir' | 'category' | 'diff'>;

interface Props {
  rect: TooltipSubject;
  generation: number;
  /** Size of the focused folder, for the share line. */
  viewSize: number;
  /** Cursor position inside the treemap, CSS pixels. */
  x: number;
  y: number;
  /** Place the tooltip left of / above the cursor (near the right / bottom edge). */
  flipX: boolean;
  flipY: boolean;
}

/** Positioned tooltip with the full path, size and share of the hovered rectangle. */
export function TreemapTooltip({ rect, generation, viewSize, x, y, flipX, flipY }: Props) {
  const [, refresh] = useReducer((n: number) => n + 1, 0);
  // The merged "other" arc of the sunburst is not a node: no path, no kind.
  const merged = rect.id === OTHER_ARC_ID;
  const path = merged ? undefined : cachedPath(generation, rect.id);
  const tone = diffTone(rect.diff);

  // Fetch the path only once the pointer rests on a rectangle for a moment.
  useEffect(() => {
    if (merged || cachedPath(generation, rect.id) !== undefined) return;
    let alive = true;
    const timer = window.setTimeout(() => {
      loadPath(generation, rect.id).then(
        () => alive && refresh(),
        () => {},
      );
    }, PATH_DELAY_MS);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [generation, rect.id, merged]);

  const offset = 14;
  const style = {
    left: flipX ? undefined : x + offset,
    right: flipX ? `calc(100% - ${x - offset}px)` : undefined,
    top: flipY ? undefined : y + offset,
    bottom: flipY ? `calc(100% - ${y - offset}px)` : undefined,
  };
  const share = viewSize > 0 ? (100 * rect.size) / viewSize : 0;

  return (
    <div className="tooltip" style={style} role="tooltip">
      <div className="tooltip-name">
        <span className="tooltip-kind">{rect.is_dir ? strings.folder : strings.file}</span>
        {rect.name}
      </div>
      <div className="tooltip-path">{path ?? '…'}</div>
      {rect.category && (
        <div className="tooltip-category">
          {strings.tooltipCategory(strings.categoryNames[rect.category])}
        </div>
      )}
      <div className="tooltip-size">
        <strong>{formatBytes(rect.size)}</strong>
        <span>
          {formatPercent(share)} {strings.ofView}
        </span>
      </div>
      {tone && rect.diff && (
        <div className="tooltip-diff" data-tone={tone}>
          {tone === 'added'
            ? strings.tooltipNew(formatDelta(rect.diff.delta))
            : strings.tooltipChanged(formatDelta(rect.diff.delta))}
        </div>
      )}
    </div>
  );
}
