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

// Text on gradient buttons, accent fills and the title bar, checked against every gradient stop.
const labelPairs: Record<string, [label: string, backgrounds: string[]][]> = {
  silver: [
    ['#ffffff', ['#505967', '#323842', '#424b59', '#272b33']], // --on-btn on --btn-grad(-hover)
    ['#ffffff', ['#475569']], // --on-accent on --accent
    ['#191c22', ['#f8f9fb', '#edf0f3', '#e8eaee']], // --chrome-text on --chrome-bg
    ['#191c22', ['#e2e5eb']], // --chrome-text on --chrome-hover
    ['#ffffff', ['#ac2632']], // --on-close-hover on --close-hover
  ],
  graphite: [
    ['#141518', ['#f3f4f6', '#b8bec9', '#ffffff', '#d4dae3']], // --on-btn on --btn-grad(-hover)
    ['#141518', ['#c7d0df']], // --on-accent on --accent
    ['#f3f4f6', ['#353941', '#22252a', '#18191c']], // --chrome-text on --chrome-bg
    ['#f3f4f6', ['#474c55']], // --chrome-text on --chrome-hover
    ['#ffffff', ['#ac2632']], // --on-close-hover on --close-hover
  ],
};

describe('contrastRatio', () => {
  it('computes known ratios', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 1);
    expect(contrastRatio('#777777', '#777777')).toBeCloseTo(1, 5);
  });
});

describe('button, accent and title bar label contrast', () => {
  for (const [theme, groups] of Object.entries(labelPairs)) {
    for (const [label, backgrounds] of groups) {
      for (const background of backgrounds) {
        it(`${theme} ${label} on ${background} meets AA`, () => {
          expect(contrastRatio(label, background)).toBeGreaterThanOrEqual(4.5);
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
