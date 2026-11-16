import { describe, expect, it } from 'vitest';
import { contrastRatio } from './contrast';
import { HUES, labelColors, treemapColor, type Role, type Theme } from './treemapPaint';

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
    ['dark tertiary on bg', '#7C9690', '#0B1226'],
    ['dark tertiary on bg bottom', '#7C9690', '#0F1F2A'],
    ['dark tertiary on surface', '#7C9690', '#13202E'],
    ['light secondary on bg', '#4A625B', '#F3F8F6'],
    ['light secondary on bg bottom', '#4A625B', '#E8F1EE'],
    ['light secondary on surface', '#4A625B', '#FFFFFF'],
    ['light tertiary on bg', '#596D67', '#F3F8F6'],
    ['light tertiary on bg bottom', '#596D67', '#E8F1EE'],
    ['light tertiary on surface', '#596D67', '#FFFFFF'],
  ])('%s is at least 4.5:1 (secondary text is real text)', (_name, fg, bg) => {
    expect(contrastRatio(fg, bg)).toBeGreaterThanOrEqual(4.5);
  });
});

describe('treemap label contrast', () => {
  const themes: Theme[] = ['light', 'dark'];
  const roles: Role[] = ['file', 'header'];
  for (const theme of themes) {
    for (const role of roles) {
      it(`${theme} ${role} labels meet AA for every hue and depth`, () => {
        for (let slot = 0; slot < HUES.length; slot += 1) {
          for (let depth = 1; depth <= 12; depth += 1) {
            const fill = treemapColor(theme, slot, depth, role);
            const label = labelColors(fill);
            expect(contrastRatio(fill, label.main), `${fill} main`).toBeGreaterThanOrEqual(4.5);
            expect(contrastRatio(fill, label.sub), `${fill} sub`).toBeGreaterThanOrEqual(3);
          }
        }
      });
    }
  }
});
