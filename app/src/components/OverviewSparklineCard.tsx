import { useId } from 'react';
import { OVERVIEW_HISTORY_LIMIT, formatWholePercent, sparklineGeometry } from '../overviewModel';
import { strings } from '../strings';

const VIEW_W = 320;
const VIEW_H = 88;

/** Live 60-second area-sparkline card with smooth inline SVG polyline and vertical gradient fill. */
export function OverviewSparklineCard({
  title,
  subtitle,
  currentPct,
  samples,
  tone = 'accent',
}: {
  title: string;
  subtitle: string;
  currentPct: number | null;
  samples: readonly number[];
  tone?: 'accent' | 'silver';
}) {
  const gradId = `spark-${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const geom = sparklineGeometry(samples, VIEW_W, VIEW_H, OVERVIEW_HISTORY_LIMIT, 6, 6);

  return (
    <section className={`overview-card sparkline-card tone-${tone}`} aria-label={title}>
      <header className="sparkline-header">
        <div className="sparkline-titles">
          <h2>{title}</h2>
          <span>{subtitle}</span>
        </div>
        <strong className="sparkline-value">
          {currentPct !== null ? formatWholePercent(currentPct) : '—'}
        </strong>
      </header>

      <div className="sparkline-stage" aria-hidden="true">
        <svg viewBox={`0 0 ${VIEW_W} ${VIEW_H}`} preserveAspectRatio="none">
          <defs>
            <linearGradient id={gradId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" className="spark-stop-top" />
              <stop offset="100%" className="spark-stop-bottom" />
            </linearGradient>
          </defs>
          <line className="spark-guide" x1="0" y1="22" x2={VIEW_W} y2="22" vectorEffect="non-scaling-stroke" />
          <line className="spark-guide" x1="0" y1="44" x2={VIEW_W} y2="44" vectorEffect="non-scaling-stroke" />
          <line className="spark-guide" x1="0" y1="66" x2={VIEW_W} y2="66" vectorEffect="non-scaling-stroke" />
          {geom.areaPath && <path className="spark-area" d={geom.areaPath} fill={`url(#${gradId})`} />}
          {geom.linePoints && (
            <polyline
              className="spark-line"
              points={geom.linePoints}
              vectorEffect="non-scaling-stroke"
            />
          )}
        </svg>
        {geom.lastPoint && (
          <span
            className="spark-dot"
            style={{
              left: `${(geom.lastPoint.x / VIEW_W) * 100}%`,
              top: `${(geom.lastPoint.y / VIEW_H) * 100}%`,
            }}
          />
        )}
      </div>

      <footer className="sparkline-axis">
        <span>{strings.overviewHistoryStart}</span>
        <span>{strings.overviewHistoryNow}</span>
      </footer>
    </section>
  );
}
