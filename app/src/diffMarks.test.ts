import { describe, expect, it } from 'vitest';
import type { NodeDiff } from './api';
import { computeMarkLevels, diffLabels, diffTone, markStyle, toneColor } from './diffMarks';

const diff = (status: NodeDiff['status'], delta: number): NodeDiff => ({
  status,
  base: status === 'added' ? null : 100,
  delta,
  removed: null,
});

describe('diff tone', () => {
  it('marks only real changes', () => {
    expect(diffTone(null)).toBeNull();
    expect(diffTone(undefined)).toBeNull();
    expect(diffTone(diff('unchanged', 0))).toBeNull();
    expect(diffTone(diff('grown', 5))).toBe('grown');
    expect(diffTone(diff('shrunk', -5))).toBe('shrunk');
    expect(diffTone(diff('added', 5))).toBe('added');
  });

  it('keeps the sign and a New word in the labels so colour is not the only cue', () => {
    expect(diffLabels(diff('grown', 4_800_000_000))).toEqual(['+4.5 GB']);
    expect(diffLabels(diff('shrunk', -2048))[0].startsWith('−')).toBe(true);
    expect(diffLabels(diff('added', 2048))).toEqual(['New +2.0 KB', 'New']);
    expect(diffLabels(null)).toEqual([]);
    expect(diffLabels(diff('unchanged', 0))).toEqual([]);
  });

  it('picks the grow colour for new and grown items and the shrink colour for shrunk ones', () => {
    const colors = { grow: '#a00000', shrink: '#00a000', halo: '#ffffff' };
    expect(toneColor('grown', colors)).toBe('#a00000');
    expect(toneColor('added', colors)).toBe('#a00000');
    expect(toneColor('shrunk', colors)).toBe('#00a000');
  });
});

describe('mark style', () => {
  it('gives each kind of change its own line pattern', () => {
    const grown = markStyle('grown', 1);
    const shrunk = markStyle('shrunk', 1);
    const added = markStyle('added', 1);
    expect(grown.dash).toEqual([]);
    expect(shrunk.dash.length).toBe(2);
    expect(added.dash.length).toBe(2);
    expect(added.cap).toBe('round');
    expect(shrunk.dash[0]).toBeGreaterThan(added.dash[0] * 100);
    expect(new Set([grown, shrunk, added].map((s) => JSON.stringify(s))).size).toBe(3);
  });

  it('uses thinner lines for folders that only contain changes and scales with dpr', () => {
    expect(markStyle('grown', 1, false).width).toBeLessThan(markStyle('grown', 1, true).width);
    expect(markStyle('grown', 2, true).width).toBe(4);
    expect(markStyle('shrunk', 2).dash).toEqual([16, 8]);
  });
});

describe('computeMarkLevels', () => {
  it('marks a change strongly and the folders above it thinly', () => {
    // 0 root, 1 dir (grown), 2 dir (grown), 3 file (added), 4 file unchanged, 5 file shrunk.
    const parents = [-1, 0, 1, 2, 1, 0];
    const tones = [null, 'grown', 'grown', 'added', null, 'shrunk'] as const;
    expect(Array.from(computeMarkLevels(parents, tones))).toEqual([0, 1, 1, 2, 0, 2]);
  });

  it('treats a changed leaf folder as a strong mark and ignores unchanged branches', () => {
    expect(Array.from(computeMarkLevels([-1, 0, 0], [null, 'grown', null]))).toEqual([0, 2, 0]);
    expect(Array.from(computeMarkLevels([-1, 0], [null, null]))).toEqual([0, 0]);
    expect(computeMarkLevels([], []).length).toBe(0);
  });

  it('does not trust out-of-range parent indexes', () => {
    expect(Array.from(computeMarkLevels([-1, 99], [null, 'grown']))).toEqual([0, 2]);
  });
});
