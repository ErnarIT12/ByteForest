import { describe, expect, it } from 'vitest';

/** WCAG 2.x relative luminance of a `#rrggbb` colour. */
function luminance(hex: string): number {
  const n = Number.parseInt(hex.slice(1), 16);
  const lin = (c: number) => {
    const v = c / 255;
    return v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin((n >> 16) & 255) + 0.7152 * lin((n >> 8) & 255) + 0.0722 * lin(n & 255);
}

/** WCAG contrast ratio between two `#rrggbb` colours (1..21). */
export function contrastRatio(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

// Values are copied from the spec / styles.css tokens; review keeps them in sync.
describe('brand theme contrast', () => {
  it('computes known ratios', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 5);
    expect(contrastRatio('#777777', '#777777')).toBeCloseTo(1, 5);
  });

  it.each([
    ['dark text on bg', '#E6F4F1', '#0B1226'],
    ['dark text on bg bottom', '#E6F4F1', '#0F1F2A'],
    ['dark text on surface', '#E6F4F1', '#13202E'],
    ['dark text on raised', '#E6F4F1', '#182A38'],
    ['button text on green', '#062018', '#5EDB7F'],
    ['button text on teal', '#062018', '#1CAE99'],
    ['dark accent on bg', '#5EDB7F', '#0B1226'],
    ['light text on bg', '#0E1F1A', '#F3F8F6'],
    ['light text on surface', '#0E1F1A', '#FFFFFF'],
    ['on-accent on light accent', '#FFFFFF', '#187A50'],
    ['light accent on bg', '#187A50', '#F3F8F6'],
  ])('%s is at least 4.5:1', (_name, fg, bg) => {
    expect(contrastRatio(fg, bg)).toBeGreaterThanOrEqual(4.5);
  });

  it.each([
    ['dark secondary on bg', '#9DB7B2', '#0B1226'],
    ['dark secondary on surface', '#9DB7B2', '#13202E'],
    ['dark tertiary on surface', '#7C9690', '#13202E'],
    ['light secondary on bg', '#4A625B', '#F3F8F6'],
    ['light tertiary on surface', '#6B817B', '#FFFFFF'],
  ])('%s is at least 3:1', (_name, fg, bg) => {
    expect(contrastRatio(fg, bg)).toBeGreaterThanOrEqual(3);
  });
});
