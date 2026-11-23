import { describe, expect, it } from 'vitest';
import { contrastRatio, mixSrgb } from './contrast';
import {
  CATEGORIES,
  HUES,
  categoryColor,
  labelColors,
  treemapColor,
  type Role,
  type Theme,
} from './treemapPaint';

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

describe('mixSrgb', () => {
  it('blends like color-mix in srgb', () => {
    expect(mixSrgb('#000000', '#ffffff', 50)).toBe('#808080');
    expect(mixSrgb('#ff0000', '#0000ff', 0)).toBe('#0000ff');
    expect(mixSrgb('#ff0000', '#0000ff', 100)).toBe('#ff0000');
    expect(mixSrgb('#a62839', '#f6f7f9', 14)).toBe('#ebdade');
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

// Change text (--delta-grow, --delta-shrink) on the surfaces it appears on: panels, raised
// areas, the selected row (accent mixed into the surface), hovered rows and the compare banner.
const deltaInks = {
  light: ['#9a3412', '#17663a'],
  dark: ['#ffb27a', '#7fd6a0'],
};
const deltaSurfaces = {
  light: ['#ffffff', '#f6f7f9', '#edf0f3', '#e8eaee', '#e0e2e6', '#eeeff0', '#ecedef'],
  dark: ['#202226', '#292c32', '#17181b', '#3c4046', '#2f3238', '#31343a'],
};

describe('snapshot change colour contrast', () => {
  for (const theme of ['light', 'dark'] as const) {
    for (const ink of deltaInks[theme]) {
      for (const surface of deltaSurfaces[theme]) {
        it(`${theme} ${ink} on ${surface} meets AA`, () => {
          expect(contrastRatio(ink, surface)).toBeGreaterThanOrEqual(4.5);
        });
      }
    }
  }
});

// Destructive text: `.context-menu-item.danger` (on the raised menu surface, and on a 14% tint
// of --danger over it when hovered or focused) and `.btn.danger` (on --surface, --control and
// the 14% tint over --surface when hovered), see contextMenu.css.
const danger = {
  light: {
    ink: '#a62839',
    surface: '#ffffff',
    raised: '#f6f7f9',
    control: '#f0f1f4',
    controlHover: '#e2e5eb',
  },
  dark: {
    ink: '#ff9ea9',
    surface: '#202226',
    raised: '#292c32',
    control: '#30333a',
    controlHover: '#3a3e46',
  },
};

describe('danger colour contrast', () => {
  for (const theme of ['light', 'dark'] as const) {
    const d = danger[theme];
    const backgrounds: [string, string][] = [
      ['menu item', d.raised],
      ['menu item hover', mixSrgb(d.ink, d.raised, 14)],
      ['btn on surface', d.surface],
      ['btn on control', d.control],
      ['btn hover tint', mixSrgb(d.ink, d.surface, 14)],
      ['btn on control hover', d.controlHover],
    ];
    for (const [what, background] of backgrounds) {
      it(`${theme} danger text on ${what} (${background}) meets AA`, () => {
        expect(contrastRatio(d.ink, background)).toBeGreaterThanOrEqual(4.5);
      });
    }
  }
});

// Sidebar, segmented switch, badges and the empty-state card (v0.4): text on the selected
// row (accent mixed into the surface), on hover tints over the page background, on the
// raised strips that replaced bordered toolbars, and on the control track of the switch.
const v04 = {
  light: {
    text: '#191c22',
    text2: '#424954',
    text3: '#57606d',
    accent: '#475569',
    surface: '#ffffff',
    raised: '#f6f7f9',
    control: '#f0f1f4',
    pages: ['#f8f9fb', '#edf0f3', '#e8eaee'],
    barFill: '#69788c',
    barTrack: '#d5d8df',
  },
  dark: {
    text: '#f3f4f6',
    text2: '#c6cad2',
    text3: '#a0a7b2',
    accent: '#c7d0df',
    surface: '#202226',
    raised: '#292c32',
    control: '#30333a',
    pages: ['#23252a', '#17181b', '#141518'],
    barFill: '#929eaf',
    barTrack: '#383b42',
  },
};

describe('sidebar and flat panel contrast', () => {
  for (const theme of ['light', 'dark'] as const) {
    const t = v04[theme];
    const selected = mixSrgb(t.accent, t.surface, 17);
    it(`${theme} text on the selected row (${selected}) meets AA`, () => {
      for (const ink of [t.text, t.text2, t.accent]) {
        expect(contrastRatio(ink, selected)).toBeGreaterThanOrEqual(4.5);
      }
    });
    it(`${theme} sidebar text on hovered rows over the page meets AA`, () => {
      for (const page of t.pages) {
        const hover = mixSrgb(t.text, page, 7);
        for (const ink of [t.text, t.text2]) {
          expect(contrastRatio(ink, hover), `${ink} on ${hover}`).toBeGreaterThanOrEqual(4.5);
        }
        expect(contrastRatio(t.text3, page)).toBeGreaterThanOrEqual(4.5);
      }
    });
    it(`${theme} text on raised strips and the switch track meets AA`, () => {
      for (const ink of [t.text, t.text2, t.text3]) {
        expect(contrastRatio(ink, t.raised)).toBeGreaterThanOrEqual(4.5);
        expect(contrastRatio(ink, t.surface)).toBeGreaterThanOrEqual(4.5);
      }
      for (const ink of [t.text2, t.text]) {
        expect(contrastRatio(ink, t.control)).toBeGreaterThanOrEqual(4.5);
      }
      expect(contrastRatio(t.text, selected)).toBeGreaterThanOrEqual(4.5);
    });
    it(`${theme} usage meter fill is visible on its track`, () => {
      expect(contrastRatio(t.barFill, t.barTrack)).toBeGreaterThanOrEqual(3);
    });
    it(`${theme} selected row and accent indicators stand out from the page`, () => {
      for (const page of t.pages) {
        expect(contrastRatio(t.accent, page)).toBeGreaterThanOrEqual(3);
      }
    });
  }
});

// Discard Pile (v0.4): the treemap marker (--tm-discard) is a non-text graphic and must stand
// out from every fill it can be drawn on, in both colour modes; the dock's list rows put text
// on the hover and pressed tints over the dock surface.
const discardMarker: Record<Theme, string> = { light: '#4a0d18', dark: '#ffe9bd' };

describe('Discard Pile marker contrast', () => {
  for (const theme of ['light', 'dark'] as const) {
    it(`${theme} marker meets 3:1 on every treemap fill`, () => {
      const marker = discardMarker[theme];
      const roles: Role[] = ['file', 'dirFill'];
      for (const role of roles) {
        for (let slot = 0; slot < HUES.length; slot += 1) {
          for (let depth = 1; depth <= 12; depth += 1) {
            const fill = treemapColor(theme, slot, depth, role);
            expect(contrastRatio(marker, fill), `${role} ${fill}`).toBeGreaterThanOrEqual(3);
          }
        }
        for (const category of [...CATEGORIES, null]) {
          const fill = categoryColor(theme, category, role);
          expect(contrastRatio(marker, fill), `${role} ${category} ${fill}`).toBeGreaterThanOrEqual(3);
        }
      }
    });

    it(`${theme} dock text on hovered and pressed rows meets AA`, () => {
      const t = v04[theme];
      for (const [tint, amount] of [['hover', 7], ['pressed', 12]] as const) {
        const row = mixSrgb(t.text, t.surface, amount);
        for (const ink of [t.text, t.text2, t.text3]) {
          expect(contrastRatio(ink, row), `${tint} ${ink} on ${row}`).toBeGreaterThanOrEqual(4.5);
        }
      }
    });
  }
});

// Map change marks (compare mode): an outline in --delta-grow / --delta-shrink laid on a halo
// of --tm-sel-inner, so the outline always meets its neighbour at 3:1 whatever the tile colour
// below it; the pill label sits on the mark colour; the legend swatch on the raised strip; and
// the mark against the map backdrop (--tm-bg) that shows between tiles.
const marks = {
  light: { grow: '#9a3412', shrink: '#17663a', halo: '#ffffff', backdrop: '#d8dce2', raised: '#f6f7f9' },
  dark: { grow: '#ffb27a', shrink: '#7fd6a0', halo: '#141518', backdrop: '#181a1e', raised: '#292c32' },
};

describe('map change mark contrast', () => {
  for (const theme of ['light', 'dark'] as const) {
    const m = marks[theme];
    for (const kind of ['grow', 'shrink'] as const) {
      it(`${theme} ${kind} outline keeps 3:1 against its halo, the backdrop and the legend strip`, () => {
        expect(contrastRatio(m[kind], m.halo)).toBeGreaterThanOrEqual(3);
        expect(contrastRatio(m[kind], m.backdrop)).toBeGreaterThanOrEqual(3);
        expect(contrastRatio(m[kind], m.raised)).toBeGreaterThanOrEqual(3);
      });
      it(`${theme} ${kind} pill text meets AA on the mark colour`, () => {
        expect(contrastRatio(m[kind], labelColors(m[kind]).main)).toBeGreaterThanOrEqual(4.5);
      });
    }
  }
});
