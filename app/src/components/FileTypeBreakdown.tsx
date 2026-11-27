import { useEffect, useState } from 'react';
import { folderBreakdown, type Breakdown, type FileCategory } from '../api';
import { formatBytes, formatInt, formatPercent } from '../format';
import { useColorScheme } from '../hooks/useColorScheme';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { categoryColor } from '../treemapPaint';

const TOP_EXTENSIONS = 8;

interface Loaded {
  key: string;
  breakdown?: Breakdown;
  error?: string;
}

/** File types inside a folder: category bars and the largest extensions; rows open the search. */
export function FileTypeBreakdown({ scanId, id, name }: { scanId: number; id: number; name: string }) {
  const theme = useColorScheme();
  const key = `${scanId}:${id}`;
  const [loaded, setLoaded] = useState<Loaded | null>(null);

  useEffect(() => {
    let alive = true;
    folderBreakdown(scanId, id, TOP_EXTENSIONS).then(
      (breakdown) => { if (alive) setLoaded({ key, breakdown }); },
      (error: unknown) => { if (alive) setLoaded({ key, error: String(error) }); },
    );
    return () => { alive = false; };
  }, [scanId, id, key]);

  const current = loaded?.key === key ? loaded : null;
  const breakdown = current?.breakdown;
  const search = (filters: { category?: FileCategory; extension?: string }) =>
    useAppStore.getState().openSearch({ ...filters, under: { id, name } });
  const share = (bytes: number) =>
    breakdown && breakdown.total_bytes > 0 ? (100 * bytes) / breakdown.total_bytes : 0;

  return (
    <section className="file-types inspector-card" aria-label={strings.fileTypes}>
      <h3>{strings.fileTypes}</h3>
      {!current && <p className="muted" role="status">{strings.fileTypesLoading}</p>}
      {current?.error && <p className="muted" role="alert">{strings.fileTypesFailed}</p>}
      {breakdown && breakdown.total_files > 0 && (
        <>
          <ul className="type-rows">
            {breakdown.categories.filter((c) => c.bytes > 0 || c.files > 0).map((c) => {
              const label = strings.categoryNames[c.category];
              const fill = categoryColor(theme, c.category, 'file');
              return (
                <li key={c.category}>
                  <button type="button" className="type-row"
                    aria-label={strings.breakdownRow(label, formatBytes(c.bytes), formatPercent(share(c.bytes)))}
                    onClick={() => search({ category: c.category })}>
                    <span className="type-name">
                      <span className="legend-swatch" aria-hidden="true" style={{ background: fill }} />
                      {label}
                    </span>
                    <span className="type-size">{formatBytes(c.bytes)}</span>
                    <span className="type-share">{formatPercent(share(c.bytes))}</span>
                    <span className="type-bar" aria-hidden="true">
                      <span style={{ width: `${Math.min(100, share(c.bytes))}%`, background: fill }} />
                    </span>
                  </button>
                </li>
              );
            })}
            {breakdown.folder_bytes > 0 && (
              <li>
                <div className="type-row static">
                  <span className="type-name">{strings.folderMetadata}</span>
                  <span className="type-size">{formatBytes(breakdown.folder_bytes)}</span>
                  <span className="type-share">{formatPercent(share(breakdown.folder_bytes))}</span>
                </div>
              </li>
            )}
          </ul>
          {breakdown.extensions.length > 0 && (
            <>
              <h4>{strings.topExtensions}</h4>
              <ul className="type-rows">
                {breakdown.extensions.map((e) => {
                  const body = (
                    <>
                      <span className="type-name">{e.ext ? `.${e.ext}` : strings.noExtension}</span>
                      <span className="type-size">{formatBytes(e.bytes)}</span>
                      <span className="type-share">{formatInt(e.files)}</span>
                    </>
                  );
                  return (
                    <li key={e.ext}>
                      {e.ext ? (
                        <button type="button" className="type-row" onClick={() => search({ extension: e.ext })}>
                          {body}
                        </button>
                      ) : (
                        <div className="type-row static">{body}</div>
                      )}
                    </li>
                  );
                })}
              </ul>
            </>
          )}
        </>
      )}
    </section>
  );
}
