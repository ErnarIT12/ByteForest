import { describe, expect, it } from 'vitest';
import { deltaCell } from './diffView';

describe('deltaCell', () => {
  it('shows nothing without a comparison or when nothing changed', () => {
    expect(deltaCell(undefined)).toBeNull();
    expect(deltaCell(null)).toBeNull();
    expect(deltaCell({ status: 'unchanged', base: 5, delta: 0, removed: null })).toBeNull();
  });

  it('badges new entries instead of printing their size', () => {
    expect(deltaCell({ status: 'added', base: null, delta: 4096, removed: null })).toEqual({
      text: 'New',
      tone: 'added',
    });
  });

  it('prints signed sizes for grown and shrunk entries', () => {
    expect(deltaCell({ status: 'grown', base: 1, delta: 2048, removed: null })).toEqual({
      text: '+2.0 KB',
      tone: 'grown',
    });
    expect(deltaCell({ status: 'shrunk', base: 9, delta: -1536, removed: null })).toEqual({
      text: '−1.5 KB',
      tone: 'shrunk',
    });
  });
});
