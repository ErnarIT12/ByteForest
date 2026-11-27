import { useEffect, useMemo, useState, type CSSProperties } from 'react';
import {
  folderBreakdown,
  revealNodeInFileManager,
  searchFiles,
  type Breakdown,
  type FileCategory,
  type FileFilter,
  type FileMatch,
  type SearchKind,
  type SearchResults,
} from '../api';
import { toggleQueued, useDiscardPile } from '../discardPile';
import { formatBytes, formatInt } from '../format';
import { useColorScheme } from '../hooks/useColorScheme';
import { SEARCH_PRESET_IDS, presetFilter, type SearchPresetId } from '../searchPresets';
import { useAppStore, type SearchPreset } from '../store';
import { strings } from '../strings';
import { CATEGORIES, categoryColor, type Theme } from '../treemapPaint';
import { FileKindIcon } from './FileKindIcon';
import {
  Box,
  Database,
  Disc,
  FolderOpen,
  GitBranch,
  HardDrive,
  Icon,
  Layers,
  Package,
  Search,
  Trash2,
  X,
  type LucideIcon,
} from './Icon';

const MIN_SIZES = [
  { label: 'Any', value: 0 },
  { label: '10 MB+', value: 10 * 1024 ** 2 },
  { label: '100 MB+', value: 100 * 1024 ** 2 },
  { label: '1 GB+', value: 1024 ** 3 },
  { label: '10 GB+', value: 10 * 1024 ** 3 },
] as const;

const PRESET_META: Record<SearchPresetId, { icon: LucideIcon; hue: Record<Theme, string> }> = {
  caches: { icon: Layers, hue: { light: '#b45309', dark: '#f59e0b' } },
  node_modules: { icon: Package, hue: { light: '#2563eb', dark: '#60a5fa' } },
  target: { icon: Box, hue: { light: '#c2410c', dark: '#fb923c' } },
  venv: { icon: Database, hue: { light: '#0369a1', dark: '#38bdf8' } },
  git: { icon: GitBranch, hue: { light: '#be123c', dark: '#fb7185' } },
  diskImages: { icon: Disc, hue: { light: '#0f766e', dark: '#2dd4bf' } },
  huge: { icon: HardDrive, hue: { light: '#475569', dark: '#cbd5e1' } },
};

/** Parent directory of `fullPath` without repeating `name` at the end. */
function parentPathOf(fullPath: string, name: string): string {
  if (name.length > 0 && fullPath.length > name.length) {
    const cut = fullPath.length - name.length;
    const sep = fullPath[cut - 1];
    if ((sep === '\\' || sep === '/') && fullPath.endsWith(name)) {
      const parent = fullPath.slice(0, cut - 1);
      if (parent === '') return '/';
      if (/^[A-Za-z]:$/.test(parent)) return `${parent}${sep}`;
      return parent;
    }
  }
  const slash = Math.max(fullPath.lastIndexOf('\\'), fullPath.lastIndexOf('/'));
  if (slash === 0) return '/';
  if (slash > 0) {
    const parent = fullPath.slice(0, slash);
    return /^[A-Za-z]:$/.test(parent) ? `${parent}${fullPath[slash]}` : parent;
  }
  return fullPath;
}

interface RequestResult {
  generation: number;
  filter: FileFilter;
  results?: SearchResults;
  error?: string;
}

interface PresetStat {
  total: number;
  bytes: number;
}

interface FileSearchProps {
  scanId: number;
  active: boolean;
  onOpen: (id: number) => void;
  opening: boolean;
  navigationError: string | null;
}

function FileMatchRow({ file, scanId, isLive, writable, revealLabel, onOpen, onError }: {
  file: FileMatch;
  scanId: number;
  isLive: boolean;
  writable: boolean;
  revealLabel: string;
  onOpen: (id: number) => void;
  onError: (message: string | null) => void;
}) {
  const isQueued = useDiscardPile((s) => s.idSet.has(file.id));
  const busy = useDiscardPile((s) => s.busy);
  const parentDir = parentPathOf(file.path, file.name);
  return (
    <tr className={isQueued ? 'is-queued' : undefined}>
      <td>
        <div className="match-lead">
          {writable && (
            <input
              type="checkbox"
              checked={isQueued}
              disabled={busy}
              aria-label={`Select ${file.path} for cleanup`}
              onChange={() => toggleQueued(file.id)}
            />
          )}
          <button
            type="button"
            className="match-name"
            onClick={() => onOpen(file.id)}
            title={`Show in disk map: ${file.name}`}
          >
            <FileKindIcon node={file.id} name={file.name} directory={file.is_dir} />
            <span className="match-title">{file.name}</span>
            {file.is_dir && <em className="result-kind">{strings.searchFolderMarker}</em>}
          </button>
        </div>
      </td>
      <td className="cell-location" title={file.path}>{parentDir}</td>
      <td className="cell-size">{formatBytes(file.size)}</td>
      <td className="cell-actions">
        <div className="row-actions">
          {isLive && (
            <button
              type="button"
              className="icon-btn row-action"
              title={revealLabel}
              aria-label={`${revealLabel}: ${file.name}`}
              onClick={() => {
                onError(null);
                void revealNodeInFileManager(scanId, file.id).catch((err: unknown) =>
                  onError(String(err)),
                );
              }}
            >
              <Icon icon={FolderOpen} size={15} />
            </button>
          )}
          {writable && (
            <button
              type="button"
              className={`icon-btn row-action ${isQueued ? 'active' : ''}`}
              disabled={busy}
              aria-pressed={isQueued}
              title={isQueued ? strings.removeFromDiscardPile : strings.addToDiscardPile}
              aria-label={`${isQueued ? strings.removeFromDiscardPile : strings.addToDiscardPile}: ${file.name}`}
              onClick={() => toggleQueued(file.id)}
            >
              <Icon icon={Trash2} size={15} />
            </button>
          )}
        </div>
      </td>
    </tr>
  );
}

function SelectionFooter() {
  const count = useDiscardPile((s) => s.ids.length);
  if (count === 0) return null;
  return (
    <footer className="selection-bar">
      <span>{formatInt(count)} selected for cleanup</span>
      <button
        type="button"
        className="btn primary"
        onClick={() => useAppStore.setState({ resultView: 'cleanup' })}
      >
        Review selection
      </button>
    </footer>
  );
}

export function FileSearch(props: FileSearchProps) {
  const preset = useAppStore((s) => s.searchPreset);
  return <FileSearchBody key={preset?.seq ?? 0} {...props} initialPreset={preset} />;
}

function FileSearchBody({ scanId, active, onOpen, opening, navigationError, initialPreset }: FileSearchProps & {
  initialPreset: SearchPreset | null;
}) {
  const theme = useColorScheme();
  const isMac = useAppStore((s) => s.isMac);
  const isLive = useAppStore((s) => s.resultSource.kind === 'live');
  const writable = useAppStore((s) => !s.scanning && s.resultSource.kind === 'live');

  const [name, setName] = useState('');
  const [extension, setExtension] = useState(() => initialPreset?.extension ?? '');
  const [minSize, setMinSize] = useState(0);
  const [category, setCategory] = useState<FileCategory | ''>(() => initialPreset?.category ?? '');
  const [scope, setScope] = useState<{ id: number; name: string } | null>(() => initialPreset?.under ?? null);
  const [kind, setKind] = useState<SearchKind>(() => (initialPreset?.caches ? 'folders' : 'files'));
  const [names, setNames] = useState<string[]>(() => (initialPreset?.caches ? presetFilter('caches').names : []));
  const [extensions, setExtensions] = useState<string[]>([]);
  const [activePreset, setActivePreset] = useState<SearchPresetId | null>(() =>
    initialPreset?.caches ? 'caches' : null,
  );
  const [actionError, setActionError] = useState<string | null>(null);

  const [response, setResponse] = useState<RequestResult | null>(null);
  const generation = useAppStore((s) => s.generation);
  const filter = useMemo<FileFilter>(
    () => ({
      name,
      extension,
      min_size: minSize,
      category: category || undefined,
      under: scope?.id,
      kind,
      names,
      extensions,
    }),
    [name, extension, minSize, category, scope, kind, names, extensions],
  );
  const current = response?.filter === filter && response.generation === generation ? response : null;

  useEffect(() => {
    if (!active || current) return;
    let alive = true;
    const timer = window.setTimeout(() => {
      searchFiles(scanId, filter).then(
        (results) => { if (alive) setResponse({ generation, filter, results }); },
        (error: unknown) => { if (alive) setResponse({ generation, filter, error: String(error) }); },
      );
    }, 150);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [active, current, filter, scanId, generation]);

  const rootId = useAppStore((s) => s.rootId);
  const scopeId = scope?.id ?? rootId;

  // File-type breakdown for the current scan/folder scope (used for dot badges and hiding 0-count types).
  const [counts, setCounts] = useState<{ key: string; breakdown: Breakdown } | null>(null);
  const countKey = `${scanId}:${scopeId}:${generation}`;
  useEffect(() => {
    if (!active || scopeId === null || counts?.key === countKey) return;
    let alive = true;
    void folderBreakdown(scanId, scopeId, 1).then((breakdown) => {
      if (alive) setCounts({ key: countKey, breakdown });
    }).catch(() => {});
    return () => { alive = false; };
  }, [active, counts, countKey, scanId, scopeId]);
  const breakdown = counts?.key === countKey ? counts.breakdown : null;

  // Reclaimable space per quick-search preset in the current scope.
  const presetKey = `${scanId}:${scope?.id ?? ''}:${generation}`;
  const [presetStats, setPresetStats] = useState<{
    key: string;
    byId: Partial<Record<SearchPresetId, PresetStat>>;
  } | null>(null);
  useEffect(() => {
    if (!active || presetStats?.key === presetKey) return;
    let alive = true;
    void Promise.all(
      SEARCH_PRESET_IDS.map(async (id) => {
        const next = presetFilter(id);
        const res = await searchFiles(scanId, {
          name: '',
          extension: '',
          min_size: next.min_size,
          under: scope?.id,
          kind: next.kind,
          names: next.names,
          extensions: next.extensions,
        });
        return [id, { total: res.total, bytes: res.total_bytes }] as const;
      }),
    )
      .then((entries) => {
        if (alive) {
          setPresetStats({ key: presetKey, byId: Object.fromEntries(entries) });
        }
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [active, presetStats, presetKey, scanId, scope]);
  const currentPresetStats = presetStats?.key === presetKey ? presetStats.byId : null;

  const results = current?.results;
  const folders = kind === 'folders';
  const edited = () => {
    setActivePreset(null);
    setActionError(null);
  };

  const resetAllFilters = () => {
    setActivePreset(null);
    setKind('files');
    setName('');
    setExtension('');
    setExtensions([]);
    setNames([]);
    setMinSize(0);
    setCategory('');
    setActionError(null);
  };

  const togglePreset = (id: SearchPresetId) => {
    setActionError(null);
    if (activePreset === id) {
      setActivePreset(null);
      setKind('files');
      setNames([]);
      setExtensions([]);
      setMinSize(0);
      return;
    }
    const next = presetFilter(id);
    setActivePreset(id);
    setKind(next.kind);
    setNames(next.names);
    setExtensions(next.extensions);
    setMinSize(next.min_size);
    setName('');
    setExtension('');
    setCategory('');
  };

  const hasFilters = Boolean(
    name || extension || minSize > 0 || category || activePreset || extensions.length > 0 || folders,
  );
  const noun = strings.searchNouns[folders ? 'folders' : 'files'];
  const revealLabel = isMac ? strings.showInFinder : strings.showInExplorer;
  const alertMessage = navigationError || actionError || current?.error;

  return (
    <section className="file-search" aria-label="Largest files">
      {scope && (
        <div className="search-scope" role="group" aria-label={strings.searchScopeLabel}>
          <span title={scope.name}>{strings.searchInFolder(scope.name)}</span>
          <button type="button" className="btn" onClick={() => setScope(null)}>
            {strings.searchClearScope}
          </button>
        </div>
      )}

      <div className="file-search-header">
        <div className="file-toolbar">
          <div className="swift-search">
            <Icon icon={Search} size={15} className="swift-search-icon" />
            <input
              type="search"
              value={name}
              maxLength={255}
              aria-label={folders ? 'Folder name' : 'File name'}
              placeholder={folders ? 'Filter folders by name…' : 'Search files by name…'}
              onChange={(event) => {
                setName(event.target.value);
                edited();
              }}
            />
            {name.length > 0 && (
              <button
                type="button"
                className="swift-clear"
                aria-label="Clear search"
                onClick={() => {
                  setName('');
                  edited();
                }}
              >
                <Icon icon={X} size={12} />
              </button>
            )}
          </div>

          {!folders && (
            <div className="swift-ext-field">
              <span className="swift-ext-prefix" aria-hidden="true">.</span>
              <input
                value={extension.replace(/^\./, '')}
                maxLength={64}
                aria-label="Extension"
                placeholder={extensions.length > 0 ? extensions.join(', ') : 'ext'}
                onChange={(event) => {
                  setExtension(event.target.value.replace(/^\./, ''));
                  setExtensions([]);
                  edited();
                }}
              />
              {(extension.length > 0 || extensions.length > 0) && (
                <button
                  type="button"
                  className="swift-clear"
                  aria-label="Clear extension"
                  onClick={() => {
                    setExtension('');
                    setExtensions([]);
                    edited();
                  }}
                >
                  <Icon icon={X} size={12} />
                </button>
              )}
            </div>
          )}

          <div className="segmented" role="group" aria-label="Minimum size">
            {MIN_SIZES.map(({ label, value }) => (
              <button
                key={value}
                type="button"
                aria-pressed={minSize === value}
                onClick={() => {
                  setMinSize(value);
                  edited();
                }}
              >
                {label}
              </button>
            ))}
          </div>

          {hasFilters && (
            <button type="button" className="btn" onClick={resetAllFilters}>
              Reset
            </button>
          )}
        </div>

        <div className="preset-strip" role="group" aria-label={strings.searchPresetsLabel}>
          {SEARCH_PRESET_IDS.map((id) => {
            const meta = PRESET_META[id];
            const stat = currentPresetStats?.[id];
            const pressed = activePreset === id;
            const style = { '--preset-hue': meta.hue[theme] } as CSSProperties;
            return (
              <button
                key={id}
                type="button"
                className="preset-card"
                style={style}
                aria-pressed={pressed}
                data-empty={stat && stat.total === 0 ? 'true' : undefined}
                onClick={() => togglePreset(id)}
              >
                <span className="preset-icon" aria-hidden="true">
                  <Icon icon={meta.icon} size={13} />
                </span>
                <span>{strings.searchPresetNames[id]}</span>
                {stat !== undefined && (
                  <span className="preset-size">{formatBytes(stat.bytes)}</span>
                )}
              </button>
            );
          })}
        </div>

        {folders ? (
          <div className="file-mode-banner">
            <span>{strings.searchFoldersHint}</span>
            <button
              type="button"
              className="btn"
              onClick={() => {
                setKind('files');
                setNames([]);
                setExtensions([]);
                edited();
              }}
            >
              {strings.searchFilesInstead}
            </button>
          </div>
        ) : (
          <div className="category-strip" role="group" aria-label={strings.searchCategory}>
            <button
              type="button"
              className="chip"
              aria-pressed={!category}
              onClick={() => {
                setCategory('');
                edited();
              }}
            >
              {strings.searchAnyType}
              {breakdown && <small>{formatInt(breakdown.total_files)}</small>}
            </button>
            {CATEGORIES.map((type) => {
              const count = breakdown?.categories.find((c) => c.category === type)?.files;
              if (breakdown && count === 0 && category !== type) return null;
              return (
                <button
                  key={type}
                  type="button"
                  className="chip"
                  aria-pressed={category === type}
                  onClick={() => {
                    setCategory(category === type ? '' : type);
                    edited();
                  }}
                >
                  <span
                    className="category-dot"
                    aria-hidden="true"
                    style={{ background: categoryColor(theme, type, 'file') }}
                  />
                  {strings.categoryNames[type]}
                  {count !== undefined && count > 0 && <small>{formatInt(count)}</small>}
                </button>
              );
            })}
          </div>
        )}
      </div>

      <div className="file-status-bar">
        <p className="file-result-summary" role="status">
          {opening
            ? 'Locating file…'
            : !current
              ? 'Searching…'
              : results &&
                (results.total === 0
                  ? strings.searchNone(folders)
                  : strings.searchSummary(
                      formatInt(results.files.length),
                      formatInt(results.total),
                      noun,
                      formatBytes(results.total_bytes),
                    ))}
        </p>
      </div>

      {alertMessage && (
        <p className="file-result-alert" role="alert">{alertMessage}</p>
      )}

      <div className="matches-scroll">
        <table className="matches files-table">
          <thead>
            <tr>
              <th scope="col" className="col-item">{folders ? 'Folder' : strings.colName}</th>
              <th scope="col" className="col-location">Location</th>
              <th scope="col" className="col-size" aria-sort="descending">{strings.colSize} ↓</th>
              <th scope="col" className="col-actions"><span className="sr-only">Actions</span></th>
            </tr>
          </thead>
          <tbody>
            {results?.files.map((file) => (
              <FileMatchRow
                key={file.id}
                file={file}
                scanId={scanId}
                isLive={isLive}
                writable={writable}
                revealLabel={revealLabel}
                onOpen={onOpen}
                onError={setActionError}
              />
            ))}
          </tbody>
        </table>
      </div>

      {writable && active && <SelectionFooter />}
    </section>
  );
}
