import { useEffect, useMemo, useState } from 'react';
import {
  searchFiles,
  type FileCategory,
  type FileFilter,
  type SearchKind,
  type SearchResults,
} from '../api';
import { formatBytes, formatInt } from '../format';
import { useAppStore } from '../store';
import { SEARCH_PRESET_IDS, presetFilter, type SearchPresetId } from '../searchPresets';
import { strings } from '../strings';
import { Check, Icon } from './Icon';
import { CATEGORIES } from '../treemapPaint';

const MIN_SIZES = [
  { label: 'Any size', value: 0 },
  { label: '10 MiB+', value: 10 * 1024 ** 2 },
  { label: '100 MiB+', value: 100 * 1024 ** 2 },
  { label: '1 GiB+', value: 1024 ** 3 },
  { label: '10 GiB+', value: 10 * 1024 ** 3 },
];

interface RequestResult {
  generation: number;
  filter: FileFilter;
  results?: SearchResults;
  error?: string;
}

export function FileSearch({ scanId, active, onOpen, opening, navigationError }: {
  scanId: number;
  active: boolean;
  onOpen: (id: number) => void;
  opening: boolean;
  navigationError: string | null;
}) {
  const [name, setName] = useState('');
  const [extension, setExtension] = useState('');
  const [minSize, setMinSize] = useState(0);
  const [category, setCategory] = useState<FileCategory | ''>('');
  const [scope, setScope] = useState<{ id: number; name: string } | null>(null);
  const [kind, setKind] = useState<SearchKind>('files');
  const [names, setNames] = useState<string[]>([]);
  const [extensions, setExtensions] = useState<string[]>([]);
  const [activePreset, setActivePreset] = useState<SearchPresetId | null>(null);
  const preset = useAppStore((s) => s.searchPreset);
  const [appliedSeq, setAppliedSeq] = useState(preset?.seq ?? 0);
  // A new preset replaces the filters (adjusting state while rendering, not in an effect).
  if (preset && preset.seq !== appliedSeq) {
    setAppliedSeq(preset.seq);
    setName('');
    setMinSize(0);
    setCategory(preset.category ?? '');
    setExtension(preset.extension ?? '');
    setScope(preset.under);
    setKind('files');
    setNames([]);
    setExtensions([]);
    setActivePreset(null);
  }
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
    }, 180);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [active, current, filter, scanId, generation]);

  const results = current?.results;
  const folders = kind === 'folders';

  const togglePreset = (id: SearchPresetId) => {
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
  const edited = () => setActivePreset(null);
  const noun = strings.searchNouns[folders ? 'folders' : 'files'];
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
      {folders && (
        <p className="file-result-summary">
          {strings.searchFoldersHint}{' '}
          <button type="button" className="btn" onClick={() => {
            setKind('files');
            setNames([]);
            setExtensions([]);
            edited();
          }}>{strings.searchFilesInstead}</button>
        </p>
      )}
      <div className="file-filters">
        <label>{folders ? 'Folder name' : 'File name'}
          <input type="search" value={name} maxLength={255}
            placeholder={folders ? 'Search folders' : 'Search files'}
            onChange={(event) => { setName(event.target.value); edited(); }} />
        </label>
        {!folders && (
          <label>Extension
            <input value={extension} maxLength={64}
              placeholder={extensions.length > 0 ? extensions.join(', ') : '.zip'}
              onChange={(event) => { setExtension(event.target.value); setExtensions([]); edited(); }} />
          </label>
        )}
        {!folders && (
          <label>{strings.searchCategory}
            <select value={category} onChange={(event) => { setCategory(event.target.value as FileCategory | ''); edited(); }}>
              <option value="">{strings.searchAnyType}</option>
              {CATEGORIES.map((c) => <option key={c} value={c}>{strings.categoryNames[c]}</option>)}
            </select>
          </label>
        )}
        <label>Minimum size
          <select value={minSize} onChange={(event) => { setMinSize(Number(event.target.value)); edited(); }}>
            {MIN_SIZES.map(({ label, value }) => <option key={value} value={value}>{label}</option>)}
          </select>
        </label>
      </div>
      <details className="quick-searches">
        <summary>Quick searches{activePreset ? ` · ${strings.searchPresetNames[activePreset]}` : ''}</summary>
      <div className="chip-row" role="group" aria-label={strings.searchPresetsLabel}>
        {SEARCH_PRESET_IDS.map((id) => (
          <button key={id} type="button" className="chip" aria-pressed={activePreset === id}
            onClick={() => togglePreset(id)}>
            {activePreset === id && <Icon icon={Check} size={12} />}
            {strings.searchPresetNames[id]}
          </button>
        ))}
      </div>
      </details>
      <p className="file-result-summary" role="status">
        {opening ? 'Locating file…' : !current ? 'Searching…' : results && (
          results.total === 0 ? strings.searchNone(folders) :
            strings.searchSummary(
              formatInt(results.files.length),
              formatInt(results.total),
              noun,
              formatBytes(results.total_bytes),
            )
        )}
      </p>
      {(navigationError || current?.error) && (
        <p className="file-result-summary" role="alert">{navigationError || current?.error}</p>
      )}
      <div className="matches-scroll">
        <table className="matches">
          <thead><tr><th scope="col">Name</th><th scope="col">Path</th><th scope="col" aria-sort="descending">Size ↓</th></tr></thead>
          <tbody>{results?.files.map((file) => (
            <tr key={file.id}>
              <td><button type="button" className="match-name" onClick={() => onOpen(file.id)}
                title={`Show in disk map: ${file.name}`}>{file.name}</button></td>
              <td title={file.path}>{file.path}</td>
              <td>{formatBytes(file.size)}</td>
            </tr>
          ))}</tbody>
        </table>
      </div>
    </section>
  );
}
