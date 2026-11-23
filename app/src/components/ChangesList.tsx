import { useEffect, useState } from 'react';
import { topChanges, type ChangeEntry } from '../api';
import { deltaCell } from '../diffView';
import { formatBytes, formatInt } from '../format';
import { useAppStore } from '../store';
import { strings } from '../strings';

const LIMIT = 200;

interface Loaded {
  key: string;
  changes?: ChangeEntry[];
  error?: string;
}

/** The biggest increases since the snapshot; a row opens the item in the tree like a search hit. */
export function ChangesList({ scanId, active, onOpen, opening, navigationError }: {
  scanId: number;
  active: boolean;
  onOpen: (id: number) => void;
  opening: boolean;
  navigationError: string | null;
}) {
  const generation = useAppStore((s) => s.generation);
  const comparing = useAppStore((s) => s.baseline !== null);
  const key = `${scanId}:${generation}`;
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const current = loaded?.key === key ? loaded : null;

  useEffect(() => {
    if (!active || !comparing || current) return;
    let alive = true;
    topChanges(scanId, LIMIT).then(
      (changes) => { if (alive) setLoaded({ key, changes }); },
      (error: unknown) => { if (alive) setLoaded({ key, error: String(error) }); },
    );
    return () => { alive = false; };
  }, [active, comparing, current, key, scanId]);

  const changes = current?.changes;
  return (
    <section className="file-search" aria-label={strings.changesMode}>
      <p className="file-result-summary" role="status">
        {opening ? 'Locating file…' : !current ? strings.changesLoading : changes && (
          changes.length === 0 ? strings.changesNone : strings.changesSummary(formatInt(changes.length))
        )}
      </p>
      {(navigationError || current?.error) && (
        <p className="file-result-summary" role="alert">
          {navigationError || `${strings.changesFailed}: ${current?.error}`}
        </p>
      )}
      <ul className="file-results">
        {changes?.map((change) => {
          const cell = deltaCell({ status: change.status, base: change.base, delta: change.delta, removed: null });
          return (
            <li key={change.id}>
              <button type="button" className="file-result" onClick={() => onOpen(change.id)} title={change.path}>
                <span>{change.name}</span>
                <strong>
                  {cell && <em className={`delta ${cell.tone}`}>{cell.text}</em>}
                  {cell?.tone === 'added' && ` ${formatBytes(change.current)}`}
                </strong>
                <small>{change.path}</small>
              </button>
            </li>
          );
        })}
      </ul>
    </section>
  );
}
