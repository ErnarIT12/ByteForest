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

// Representative Obsidian-Graphite (dark) & Frosted-Silver (light) token pairs from styles.css.
const surfaces = {
  light: ['#f5f6f8', '#eff1f5', '#ffffff', '#eef0f4'],
  dark: ['#111317', '#0d0f13', '#17191e', '#1e2128', '#232730'],
};
const inks = {
  light: ['#14171c', '#3d4450', '#565f6e', '#2563eb'],
  dark: ['#eef1f6', '#b9c0cc', '#959eab', '#60a5fa'],
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

// Text on primary CTA buttons, accent fills and the title bar, checked against every stop.
const labelPairs: Record<string, [label: string, backgrounds: string[]][]> = {
  silver: [
    ['#ffffff', ['#242831', '#181b20', '#2e333e', '#1f232b']], // --on-btn on --btn-grad(-hover)
    ['#ffffff', ['#2563eb']], // --on-accent on --accent
    ['#14171c', ['#fbfcfe', '#f3f5f8', '#f5f6f8']], // --chrome-text on --chrome-bg
    ['#14171c', ['#e3e7ee']], // --chrome-text on hover
    ['#ffffff', ['#ac2632']], // --on-close-hover on --close-hover
  ],
  graphite: [
    ['#111317', ['#f4f6fa', '#e1e6ef', '#ffffff', '#eceff4']], // --on-btn on --btn-grad(-hover)
    ['#0d1117', ['#60a5fa']], // --on-accent on --accent
    ['#eef1f6', ['#1c1f27', '#13161c', '#111317']], // --chrome-text on --chrome-bg
    ['#eef1f6', ['#2c313c']], // --chrome-text on hover
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
  light: ['#ffffff', '#f5f6f8', '#eff1f5', '#eef0f4', mixSrgb('#2563eb', '#ffffff', 9), mixSrgb('#14171c', '#ffffff', 6)],
  dark: ['#17191e', '#1e2128', '#111317', '#232730', mixSrgb('#3b82f6', '#17191e', 16), mixSrgb('#eef1f6', '#17191e', 6)],
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
// the 14% tint over --surface when hovered), see contextMenu.css & controls.css.
const danger = {
  light: {
    ink: '#a62839',
    surface: '#ffffff',
    raised: '#f5f6f8',
    control: '#eef0f4',
    controlHover: '#e3e7ee',
  },
  dark: {
    ink: '#ff9ea9',
    surface: '#17191e',
    raised: '#1e2128',
    control: '#232730',
    controlHover: '#2c313c',
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

// Sidebar, segmented switch, badges and translucent cards: text on the selected row, on hover
// tints over the page background, on raised strips, and on the control track of the switch.
const tokens = {
  light: {
    text: '#14171c',
    text2: '#3d4450',
    text3: '#565f6e',
    accent: '#2563eb',
    selectedTint: '#2563eb',
    selectedPct: 9,
    surface: '#ffffff',
    raised: '#f5f6f8',
    control: '#eef0f4',
    pages: ['#f9fafb', '#f5f6f8', '#eff1f5'],
    barFill: '#2563eb',
    barTrack: '#d9dee7',
  },
  dark: {
    text: '#eef1f6',
    text2: '#b9c0cc',
    text3: '#959eab',
    accent: '#60a5fa',
    selectedTint: '#3b82f6',
    selectedPct: 16,
    surface: '#17191e',
    raised: '#1e2128',
    control: '#232730',
    pages: ['#15181e', '#111317', '#0d0f13'],
    barFill: '#60a5fa',
    barTrack: '#2b303b',
  },
};

describe('sidebar and flat panel contrast', () => {
  for (const theme of ['light', 'dark'] as const) {
    const t = tokens[theme];
    const selected = mixSrgb(t.selectedTint, t.surface, t.selectedPct);
    it(`${theme} text on the selected row (${selected}) meets AA`, () => {
      for (const ink of [t.text, t.text2, t.accent]) {
        expect(contrastRatio(ink, selected)).toBeGreaterThanOrEqual(4.5);
      }
    });
    it(`${theme} sidebar text on hovered rows over the page meets AA`, () => {
      for (const page of t.pages) {
        const hover = mixSrgb(t.text, page, 6);
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

// Discard Pile: the treemap marker (--tm-discard) is a non-text graphic and must stand
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
      const t = tokens[theme];
      for (const [tint, amount] of [['hover', 6], ['pressed', 10]] as const) {
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
  light: { grow: '#9a3412', shrink: '#17663a', halo: '#ffffff', backdrop: '#dde1e8', raised: '#f5f6f8' },
  dark: { grow: '#ffb27a', shrink: '#7fd6a0', halo: '#111317', backdrop: '#121418', raised: '#1e2128' },
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
