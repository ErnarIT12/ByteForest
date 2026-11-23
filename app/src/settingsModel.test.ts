import { describe, expect, it } from 'vitest';
import {
  MAX_EXCLUSIONS,
  MAX_EXCLUSION_LENGTH,
  addExclusion,
  mergeSettings,
  normalizeSettings,
  removeExclusion,
} from './settingsModel';

describe('addExclusion', () => {
  it('trims and appends', () => {
    expect(addExclusion(['a'], '  node_modules ')).toEqual({ ok: true, list: ['a', 'node_modules'] });
  });

  it('rejects blank input', () => {
    expect(addExclusion([], '   ')).toEqual({ ok: false, reason: 'empty' });
  });

  it('keeps the list when the pattern is already there', () => {
    expect(addExclusion(['a', 'b'], 'a')).toEqual({ ok: true, list: ['a', 'b'] });
  });

  it('enforces the length and count limits', () => {
    expect(addExclusion([], 'x'.repeat(MAX_EXCLUSION_LENGTH + 1))).toEqual({ ok: false, reason: 'tooLong' });
    expect(addExclusion([], 'x'.repeat(MAX_EXCLUSION_LENGTH)).ok).toBe(true);
    const full = Array.from({ length: MAX_EXCLUSIONS }, (_, i) => `p${i}`);
    expect(addExclusion(full, 'new')).toEqual({ ok: false, reason: 'tooMany' });
    expect(addExclusion(full, 'p3').ok).toBe(true);
  });
});

describe('removeExclusion', () => {
  it('drops only the given pattern', () => {
    expect(removeExclusion(['a', 'b', 'c'], 'b')).toEqual(['a', 'c']);
  });
});

describe('mergeSettings', () => {
  it('keeps the fields the change did not touch', () => {
    const current = { always_elevated: true, exclusions: ['*.tmp'] };
    expect(mergeSettings(current, { exclusions: ['*.tmp', 'x'] })).toEqual({
      always_elevated: true,
      exclusions: ['*.tmp', 'x'],
    });
    expect(mergeSettings(current, { always_elevated: false })).toEqual({
      always_elevated: false,
      exclusions: ['*.tmp'],
    });
  });
});

describe('normalizeSettings', () => {
  it('fills missing fields', () => {
    expect(normalizeSettings({})).toEqual({ always_elevated: false, exclusions: [] });
    expect(normalizeSettings({ always_elevated: true })).toEqual({ always_elevated: true, exclusions: [] });
  });
});
