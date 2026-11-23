import { useEffect, useMemo, useState } from 'react';
import { searchFiles, type FileFilter, type SearchResults } from '../api';
import { formatBytes, formatInt } from '../format';

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
  const [response, setResponse] = useState<RequestResult | null>(null);
  const filter = useMemo(() => ({ name, extension, min_size: minSize }), [name, extension, minSize]);
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
      <div className="file-filters">
        <label>File name
          <input type="search" value={name} maxLength={255}
            placeholder="Search files" onChange={(event) => setName(event.target.value)} />
        </label>
        <label>Extension
          <input value={extension} maxLength={64}
            placeholder=".zip" onChange={(event) => setExtension(event.target.value)} />
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
