import { useEffect, useReducer } from 'react';
import type { Rect } from '../api';
import { formatBytes, formatPercent } from '../format';
import { cachedPath, loadPath } from '../pathCache';
import { strings } from '../strings';

/** Hover time before the full path is requested. */
const PATH_DELAY_MS = 80;

interface Props {
  rect: Rect;
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
  const path = cachedPath(generation, rect.id);

  // Fetch the path only once the pointer rests on a rectangle for a moment.
  useEffect(() => {
    if (cachedPath(generation, rect.id) !== undefined) return;
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
  }, [generation, rect.id]);

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
    </div>
  );
}
