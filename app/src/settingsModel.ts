import type { Settings } from './api';

/** Most exclusion patterns the app keeps (the backend enforces the same limits). */
export const MAX_EXCLUSIONS = 200;
/** Longest exclusion pattern, in characters. */
export const MAX_EXCLUSION_LENGTH = 260;

export type AddResult =
  | { ok: true; list: string[] }
  | { ok: false; reason: 'empty' | 'tooLong' | 'tooMany' };

/**
 * Adds a pattern to the list: trimmed, ignored when blank, unchanged when already present.
 * Rejects patterns over the length limit and lists over the size limit.
 */
export function addExclusion(list: readonly string[], text: string): AddResult {
  const pattern = text.trim();
  if (pattern === '') return { ok: false, reason: 'empty' };
  if (Array.from(pattern).length > MAX_EXCLUSION_LENGTH) return { ok: false, reason: 'tooLong' };
  if (list.includes(pattern)) return { ok: true, list: [...list] };
  if (list.length >= MAX_EXCLUSIONS) return { ok: false, reason: 'tooMany' };
  return { ok: true, list: [...list, pattern] };
}

/** The list without `pattern`. */
export const removeExclusion = (list: readonly string[], pattern: string): string[] =>
  list.filter((p) => p !== pattern);

/**
 * The full settings object to save after changing some fields. `set_settings` replaces the
 * whole file, so a change must never be sent without the fields it did not touch.
 */
export const mergeSettings = (current: Settings, patch: Partial<Settings>): Settings => ({
  ...current,
  ...patch,
});

/** Settings from the backend with every field present, whatever an older file contained. */
export const normalizeSettings = (s: Partial<Settings>): Settings => ({
  always_elevated: s.always_elevated ?? false,
  exclusions: Array.isArray(s.exclusions) ? s.exclusions : [],
});
