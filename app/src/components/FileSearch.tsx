import { useEffect, useMemo, useState } from 'react';
import { searchFiles, type FileCategory, type FileFilter, type SearchResults } from '../api';
import { formatBytes, formatInt } from '../format';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { CATEGORIES } from '../treemapPaint';

const MIN_SIZES = [
  { label: 'Any size', value: 0 },
  { label: '10 MiB+', value: 10 * 1024 ** 2 },
  { label: '100 MiB+', value: 100 * 1024 ** 2 },
  { label: '1 GiB+', value: 1024 ** 3 },
  { label: '10 GiB+', value: 10 * 1024 ** 3 },
];

interface RequestResult {
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
  }
  const [response, setResponse] = useState<RequestResult | null>(null);
  const filter = useMemo<FileFilter>(
    () => ({
      name,
      extension,
      min_size: minSize,
      category: category || undefined,
      under: scope?.id,
    }),
    [name, extension, minSize, category, scope],
  );
  const current = response?.filter === filter ? response : null;

  useEffect(() => {
    if (!active || current) return;
    let alive = true;
    const timer = window.setTimeout(() => {
      searchFiles(scanId, filter).then(
        (results) => { if (alive) setResponse({ filter, results }); },
        (error: unknown) => { if (alive) setResponse({ filter, error: String(error) }); },
      );
    }, 180);
    return () => {
      alive = false;
      window.clearTimeout(timer);
    };
  }, [active, current, filter, scanId]);

  const results = current?.results;
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
      <div className="file-filters">
        <label>File name
          <input type="search" value={name} maxLength={255}
            placeholder="Search files" onChange={(event) => setName(event.target.value)} />
        </label>
        <label>Extension
          <input value={extension} maxLength={64}
            placeholder=".zip" onChange={(event) => setExtension(event.target.value)} />
        </label>
        <label>{strings.searchCategory}
          <select value={category} onChange={(event) => setCategory(event.target.value as FileCategory | '')}>
            <option value="">{strings.searchAnyType}</option>
            {CATEGORIES.map((c) => <option key={c} value={c}>{strings.categoryNames[c]}</option>)}
          </select>
        </label>
        <label>Minimum size
          <select value={minSize} onChange={(event) => setMinSize(Number(event.target.value))}>
            {MIN_SIZES.map(({ label, value }) => <option key={value} value={value}>{label}</option>)}
          </select>
        </label>
      </div>
      <p className="file-result-summary" role="status">
        {opening ? 'Locating file…' : !current ? 'Searching…' : results && (
          results.total === 0 ? 'No matching files' :
            'Showing ' + formatInt(results.files.length) + ' of ' + formatInt(results.total) + ' files'
        )}
      </p>
      {(navigationError || current?.error) && (
        <p className="file-result-summary" role="alert">{navigationError || current?.error}</p>
      )}
      <ul className="file-results">
        {results?.files.map((file) => (
          <li key={file.id}>
            <button type="button" className="file-result" onClick={() => onOpen(file.id)} title={file.path}>
              <span>{file.name}</span>
              <strong>{formatBytes(file.size)}</strong>
              <small>{file.path}</small>
            </button>
          </li>
        ))}
      </ul>
    </section>
  );
}
