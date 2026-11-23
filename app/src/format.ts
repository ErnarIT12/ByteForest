/** Number, size and date formatting shared by every view. */

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB'] as const;

/**
 * Human-readable size, base 1024: `0 → "0 B"`, `1536 → "1.5 KB"`.
 * Bytes are shown as an integer; larger units with `decimals` digits. A value that rounds to
 * 1024 of its unit moves to the next one, so 1 048 524 (1023.95 KB) is "1.0 MB".
 */
export function formatBytes(n: number, decimals = 1): string {
  if (!Number.isFinite(n) || n <= 0) return '0 B';
  let unit = 0;
  let value = n;
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${Math.round(value)} B`;
  if (Math.round(value) >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(decimals)} ${BYTE_UNITS[unit]}`;
}

/**
 * Signed size change: `5_153_960_755 → "+4.8 GB"`, `-125_829_120 → "−120.0 MB"` (a real minus
 * sign), `0 → "0 B"`.
 */
export function formatDelta(delta: number): string {
  if (!Number.isFinite(delta) || delta === 0) return '0 B';
  return `${delta > 0 ? '+' : '−'}${formatBytes(Math.abs(delta))}`;
}

/** Share of logical bytes in a scan. Keep very small nonempty files distinguishable from zero. */
export function formatScanShare(bytes: number, total: number): string {
  if (total <= 0) return '—';
  const percent = 100 * bytes / total;
  if (percent > 0 && percent < 0.01) return '<0.01%';
  return percent.toLocaleString('en-US', { maximumFractionDigits: 2 }) + '%';
}

const COUNT_UNITS = [
  { div: 1e9, suffix: 'B' },
  { div: 1e6, suffix: 'M' },
  { div: 1e3, suffix: 'K' },
] as const;

/** Compact count: `999 → "999"`, `12345 → "12.3K"`, `1234567 → "1.2M"`. */
export function formatCount(n: number): string {
  if (!Number.isFinite(n)) return '0';
  if (Math.abs(n) < 1000) return String(Math.round(n));
  // Smallest unit first would print "1000.0K"; pick the largest unit the rounded value fits.
  for (let i = COUNT_UNITS.length - 1; i >= 0; i -= 1) {
    const { div, suffix } = COUNT_UNITS[i];
    const text = (n / div).toFixed(1);
    if (Math.abs(Number(text)) < 1000 || i === 0) return `${text}${suffix}`;
  }
  return String(n);
}

/** Exact count with thousands separators: `1234567 → "1,234,567"`. */
export function formatInt(n: number): string {
  return Math.round(n).toLocaleString('en-US');
}

/** FILETIME ticks (100 ns) between 1601-01-01 and 1970-01-01. */
const FILETIME_UNIX_EPOCH = 116_444_736_000_000_000;

/** Converts a Windows FILETIME to a `Date`; 0 (unknown) gives `null`. */
export function filetimeToDate(ft: number): Date | null {
  if (!Number.isFinite(ft) || ft <= 0) return null;
  return new Date((ft - FILETIME_UNIX_EPOCH) / 10_000);
}

const dateFormat = new Intl.DateTimeFormat('en-US', {
  year: 'numeric',
  month: 'short',
  day: 'numeric',
});

/** Short date of a FILETIME in a fixed English format ("Nov 15, 2026"), or "" when unknown. */
export function formatFiletime(ft: number): string {
  const d = filetimeToDate(ft);
  return d ? dateFormat.format(d) : '';
}

/** Short local date of a unix time in milliseconds ("Nov 15, 2026"), or "" when unusable. */
export function formatUnixMs(ms: number): string {
  if (!Number.isFinite(ms)) return '';
  const d = new Date(ms);
  return Number.isNaN(d.getTime()) ? '' : dateFormat.format(d);
}

/** Seconds with one decimal: `3420 → "3.4 s"`; below 0.05 s it is `"<0.1 s"`. */
export function formatSeconds(ms: number): string {
  const secs = Math.max(0, ms) / 1000;
  if (secs < 0.05) return '<0.1 s';
  return `${secs.toFixed(1)} s`;
}

/** Percentage with one decimal below 10 and none above: `4.25 → "4.3%"`, `42.4 → "42%"`. */
export function formatPercent(p: number): string {
  if (!Number.isFinite(p) || p <= 0) return '0%';
  if (p < 0.1) return '<0.1%';
  return p < 10 ? `${p.toFixed(1)}%` : `${Math.round(p)}%`;
}
