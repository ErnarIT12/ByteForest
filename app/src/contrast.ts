/** WCAG 2.x contrast helpers for `#rrggbb` colours. */

/** Relative luminance (0..1) of a `#rrggbb` colour. */
export function luminance(hex: string): number {
  const n = Number.parseInt(hex.slice(1), 16);
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
}

/** Contrast ratio between two `#rrggbb` colours (1..21). */
export function contrastRatio(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

/**
 * `color-mix(in srgb, fg pct%, bg)` for `#rrggbb` colours: a channel-wise linear blend in
 * gamma-encoded sRGB, rounded to the nearest 8-bit value.
 */
export function mixSrgb(fg: string, bg: string, pct: number): string {
  const f = Number.parseInt(fg.slice(1), 16);
  const b = Number.parseInt(bg.slice(1), 16);
  const t = pct / 100;
  const channel = (shift: number) => {
    const x = (f >> shift) & 255;
    const y = (b >> shift) & 255;
    return Math.round(x * t + y * (1 - t));
  };
  const out = (channel(16) << 16) | (channel(8) << 8) | channel(0);
  return `#${out.toString(16).padStart(6, '0')}`;
}
