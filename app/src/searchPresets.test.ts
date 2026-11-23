import { describe, expect, it } from 'vitest';
import { formatBytes } from './format';
import { ONE_GB, SEARCH_PRESET_IDS, presetFilter } from './searchPresets';

describe('searchPresets', () => {
  it('maps folder presets to exact-name folder searches', () => {
    expect(presetFilter('node_modules')).toMatchObject({ kind: 'folders', names: ['node_modules'] });
    expect(presetFilter('target')).toMatchObject({ kind: 'folders', names: ['target'] });
    expect(presetFilter('venv')).toMatchObject({ kind: 'folders', names: ['.venv', 'venv'] });
    expect(presetFilter('git')).toMatchObject({ kind: 'folders', names: ['.git'] });
  });

  it('maps disk images to a file extension list without a name', () => {
    const filter = presetFilter('diskImages');
    expect(filter.kind).toBe('files');
    expect(filter.names).toEqual([]);
    expect(filter.extensions).toEqual(['iso', 'vhd', 'vhdx', 'vmdk', 'qcow2', 'wim']);
  });

  it('uses a threshold that formatBytes labels as 1 GB', () => {
    const filter = presetFilter('huge');
    expect(filter).toMatchObject({ kind: 'files', min_size: ONE_GB });
    expect(formatBytes(filter.min_size)).toBe('1.0 GB');
  });

  it('returns independent copies', () => {
    const first = presetFilter('venv');
    first.names.push('x');
    expect(presetFilter('venv').names).toEqual(['.venv', 'venv']);
  });

  it('covers every preset id', () => {
    expect(SEARCH_PRESET_IDS).toHaveLength(6);
    for (const id of SEARCH_PRESET_IDS) expect(presetFilter(id)).toBeDefined();
  });
});
