import { useId } from 'react';
import { OVERVIEW_HISTORY_LIMIT, formatWholePercent, sparklineGeometry } from '../overviewModel';

const VIEW_W = 360;
const VIEW_H = 110;

/** Minimal full-width smooth SVG area-sparkline card: title top-left, live value top-right. */
export function OverviewSparklineCard({
  title,
  currentPct,
  samples,
  tone = 'accent',
}: {
  title: string;
  currentPct: number | null;
  samples: readonly number[];
  tone?: 'accent' | 'silver';
}) {
  const gradId = `spark-${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const geom = sparklineGeometry(samples, VIEW_W, VIEW_H, OVERVIEW_HISTORY_LIMIT, 12);

  return (
    <section className={`overview-card sparkline-card tone-${tone}`} aria-label={title}>
      <header className="sparkline-header">
        <h2>{title}</h2>
        <strong className="sparkline-value">
          {currentPct !== null ? formatWholePercent(currentPct) : '—'}
        </strong>
      </header>

      <div className="sparkline-canvas" aria-hidden="true">
        <svg viewBox={`0 0 ${VIEW_W} ${VIEW_H}`} preserveAspectRatio="none">
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" className="spark-stop-top" />
              <stop offset="100%" className="spark-stop-bottom" />
            </linearGradient>
          </defs>
          {geom.areaPath && <path className="spark-area" d={geom.areaPath} fill={`url(#${gradId})`} />}
          {geom.smoothLinePath && (
            <path
              className="spark-line"
              d={geom.smoothLinePath}
              vectorEffect="non-scaling-stroke"
            />
          )}
        </svg>
      </div>
    </section>
  );
}
