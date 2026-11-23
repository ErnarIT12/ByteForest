import { describe, expect, it } from 'vitest';
import { contrastRatio } from './contrast';
import { HUES, labelColors, treemapColor, type Role, type Theme } from './treemapPaint';

// Representative token pairs from styles.css, including the raised inspector surface.
const surfaces = {
  light: ['#edf0f3', '#e8eaee', '#ffffff', '#f6f7f9'],
  dark: ['#17181b', '#141518', '#202226', '#292c32'],
};
const inks = {
  light: ['#191c22', '#424954', '#57606d', '#475569'],
  dark: ['#f3f4f6', '#c6cad2', '#a0a7b2', '#c7d0df'],
};

describe('brand theme contrast', () => {
  for (const theme of ['light', 'dark'] as const) {
    for (const ink of inks[theme]) {
      for (const surface of surfaces[theme]) {
        it(`${theme} ${ink} on ${surface} meets AA`, () => {
          expect(contrastRatio(ink, surface)).toBeGreaterThanOrEqual(4.5);
        });
      }
    }
  }
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
