import type { FileFilter, SearchKind } from './api';

export type SearchPresetId = 'node_modules' | 'target' | 'venv' | 'git' | 'diskImages' | 'huge' | 'caches';

/** Binary gigabyte, the unit `formatBytes` labels "GB". */
export const ONE_GB = 1024 ** 3;

export type PresetFilter = Required<Pick<FileFilter, 'kind' | 'names' | 'extensions' | 'min_size'>>;

const FILES: SearchKind = 'files';
const FOLDERS: SearchKind = 'folders';

const DEFINITIONS: Record<SearchPresetId, PresetFilter> = {
  caches: { kind: FOLDERS, names: ['.cache', '__pycache__', '.pytest_cache'], extensions: [], min_size: 0 },
  node_modules: { kind: FOLDERS, names: ['node_modules'], extensions: [], min_size: 0 },
  target: { kind: FOLDERS, names: ['target'], extensions: [], min_size: 0 },
  venv: { kind: FOLDERS, names: ['.venv', 'venv'], extensions: [], min_size: 0 },
  git: { kind: FOLDERS, names: ['.git'], extensions: [], min_size: 0 },
  diskImages: {
    kind: FILES,
    names: [],
    extensions: ['iso', 'vhd', 'vhdx', 'vmdk', 'qcow2', 'wim'],
    min_size: 0,
  },
  huge: { kind: FILES, names: [], extensions: [], min_size: ONE_GB },
};

export const SEARCH_PRESET_IDS: readonly SearchPresetId[] = [
  'caches',
  'node_modules',
  'target',
  'venv',
  'git',
  'diskImages',
  'huge',
];

/** The filter fields a preset sets; the caller resets the rest. Returns fresh arrays. */
export function presetFilter(id: SearchPresetId): PresetFilter {
  const { kind, names, extensions, min_size } = DEFINITIONS[id];
  return { kind, names: [...names], extensions: [...extensions], min_size };
}
