/** Pure ring-buffer, usage percentage, and inline SVG sparkline helpers for the Overview dashboard. */

/** Number of 2-second samples retained (~60 seconds of live history). */
export const OVERVIEW_HISTORY_LIMIT = 30;

/** Polling interval (ms) while Overview is mounted and the document is visible. */
export const OVERVIEW_POLL_MS = 2000;

/** Clamps a finite percentage into `[0, 100]` (non-finite inputs become `0`). */
export function clampPercent(value: number): number {
  if (!Number.isFinite(value)) return 0;
  return Math.min(100, Math.max(0, value));
}

/** Appends a sample to a bounded ring buffer of at most `limit` values. */
export function pushHistorySample(
  history: readonly number[],
  value: number,
  limit = OVERVIEW_HISTORY_LIMIT,
): number[] {
  const next = [...history, clampPercent(value)];
  return next.length > limit ? next.slice(next.length - limit) : next;
}

/** Used RAM percentage (`0..100`) from `used` and `total` byte counts. */
export function memoryPercent(used: number, total: number): number {
  if (!Number.isFinite(used) || !Number.isFinite(total) || total <= 0) return 0;
  return clampPercent((used * 100) / total);
}

/** Used volume percentage (`0..100`) from `free` and `total` byte counts. */
export function drivePercent(free: number, total: number): number {
  if (!Number.isFinite(free) || !Number.isFinite(total) || total <= 0) return 0;
  const clampedFree = Math.max(0, Math.min(total, free));
  return clampPercent(((total - clampedFree) * 100) / total);
}

/** Formats a percentage as a compact integer readout (`24%`). */
export function formatWholePercent(value: number): string {
  return `${Math.round(clampPercent(value))}%`;
}

export interface SparklineGeometry {
  /** Polyline `points` attribute (`x1,y1 x2,y2 ...`), empty when `samples` is empty. */
  linePoints: string;
  /** Smooth cubic Bezier SVG `<path>` for the top stroke, empty when `samples` is empty. */
  smoothLinePath: string;
  /** Closed SVG `<path>` for the vertical gradient area fill beneath the smooth curve. */
  areaPath: string;
  /** Coordinates of the latest sample (rightmost point), or `null` when empty. */
  lastPoint: { x: number; y: number } | null;
}

/**
 * Projects `0..100` percentage samples edge-to-edge across `0..width` with smooth
 * monotonic cubic Bezier transitions (never overshooting `[0, 100]`).
 */
export function sparklineGeometry(
  samples: readonly number[],
  width: number,
  height: number,
  capacity = OVERVIEW_HISTORY_LIMIT,
  padY = 10,
  padRight = 0,
): SparklineGeometry {
  if (samples.length === 0 || width <= 0 || height <= 0) {
    return { linePoints: '', smoothLinePath: '', areaPath: '', lastPoint: null };
  }
  const visible = samples.length > capacity ? samples.slice(samples.length - capacity) : samples;
  const innerW = Math.max(1, width - padRight);
  const innerH = Math.max(1, height - padY * 2);

  if (visible.length === 1) {
    const pct = clampPercent(visible[0]);
    const y = Number((padY + innerH * (1 - pct / 100)).toFixed(2));
    const linePoints = `0,${y} ${innerW},${y}`;
    const smoothLinePath = `M 0 ${y} L ${innerW} ${y}`;
    const areaPath = `M 0 ${height} L 0 ${y} L ${innerW} ${y} L ${innerW} ${height} Z`;
    return { linePoints, smoothLinePath, areaPath, lastPoint: { x: innerW, y } };
  }

  const stepX = innerW / (visible.length - 1);
  const coords = visible.map((raw, idx) => {
    const pct = clampPercent(raw);
    const x = Number((idx * stepX).toFixed(2));
    const y = Number((padY + innerH * (1 - pct / 100)).toFixed(2));
    return { x, y };
  });

  const linePoints = coords.map((p) => `${p.x},${p.y}`).join(' ');
  const first = coords[0];
  const last = coords[coords.length - 1];
  const curveParts: string[] = [];
  for (let i = 0; i < coords.length - 1; i += 1) {
    const curr = coords[i];
    const next = coords[i + 1];
    const midX = Number(((curr.x + next.x) / 2).toFixed(2));
    curveParts.push(`C ${midX} ${curr.y}, ${midX} ${next.y}, ${next.x} ${next.y}`);
  }
  const curves = curveParts.join(' ');
  const smoothLinePath = `M ${first.x} ${first.y} ${curves}`;
  const areaPath = `M ${first.x} ${height} L ${first.x} ${first.y} ${curves} L ${last.x} ${height} Z`;
  return { linePoints, smoothLinePath, areaPath, lastPoint: last };
}
