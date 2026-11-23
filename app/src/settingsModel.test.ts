import { describe, expect, it } from 'vitest';
import {
  MAX_EXCLUSIONS,
  MAX_EXCLUSION_LENGTH,
  addExclusion,
  mergeSettings,
  normalizeAutoSnapshotKeep,
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
    const current = { always_elevated: true, exclusions: ['*.tmp'], auto_snapshot_keep: 20 };
    expect(mergeSettings(current, { exclusions: ['*.tmp', 'x'] })).toEqual({
      always_elevated: true,
      exclusions: ['*.tmp', 'x'],
      auto_snapshot_keep: 20,
    });
    expect(mergeSettings(current, { auto_snapshot_keep: 0 })).toEqual({
      always_elevated: true,
      exclusions: ['*.tmp'],
      auto_snapshot_keep: 0,
    });
  });
});

describe('normalizeSettings', () => {
  it('fills missing fields', () => {
    expect(normalizeSettings({})).toEqual({
      always_elevated: false,
      exclusions: [],
      auto_snapshot_keep: 10,
    });
    expect(normalizeSettings({ always_elevated: true }).always_elevated).toBe(true);
  });

  it('keeps the snapshot count in range and falls back for junk', () => {
    expect(normalizeSettings({ auto_snapshot_keep: 0 }).auto_snapshot_keep).toBe(0);
    expect(normalizeSettings({ auto_snapshot_keep: 99 }).auto_snapshot_keep).toBe(50);
    expect(normalizeSettings({ auto_snapshot_keep: -3 }).auto_snapshot_keep).toBe(0);
    expect(normalizeSettings({ auto_snapshot_keep: 2.5 }).auto_snapshot_keep).toBe(10);
    expect(normalizeAutoSnapshotKeep('5')).toBe(10);
  });
});
