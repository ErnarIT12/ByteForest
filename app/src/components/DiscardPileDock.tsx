import { useEffect } from 'react';
import { formatBytes, formatInt } from '../format';
import { revealQueued, useDiscardPile } from '../discardPile';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { File, Folder, Icon, Trash2, X } from './Icon';

/** `14 items · 48.3 GB` of the queue. */
function summaryLine(count: number, bytes: number): string {
  return strings.discardSummary(formatInt(count), formatBytes(bytes), count === 1);
}

/** Keeps the pile tied to the shown scan result and pruned after every change of the tree. */
export function useDiscardSync(): void {
  const resultScanId = useAppStore((s) => s.resultScanId);
  const scanning = useAppStore((s) => s.scanning);
  const generation = useAppStore((s) => s.generation);
  useEffect(() => {
    useDiscardPile.getState().bindScan(scanning ? null : resultScanId);
  }, [resultScanId, scanning]);
  useEffect(() => {
    void useDiscardPile.getState().refresh();
  }, [generation]);
}

/** Bottom dock of the sidebar: the queue of items to move to the Recycle Bin / Trash in one go. */
export function CleanupQueue() {
  const summary = useDiscardPile((s) => s.summary);
  const busy = useDiscardPile((s) => s.busy);
  const outcome = useDiscardPile((s) => s.outcome);
  const isMac = useAppStore((s) => s.isMac);
  const count = summary?.count ?? 0;
  const bytes = summary?.bytes ?? 0;
  const line = summaryLine(count, bytes);
  const moveAll = strings.discardMoveAll(isMac);

  const heading = (
    <>
      <span className="discard-icon">
        <Icon icon={Trash2} className="sidebar-icon" />
        {count > 0 && (
          <span className="discard-badge" role="img" aria-label={strings.discardBadge(formatInt(count))}>
            {count > 99 ? '99+' : count}
          </span>
        )}
      </span>
      <span className="sidebar-text">{strings.discardTitle}</span>
    </>
  );

  return (
    <section className="discard-dock" aria-labelledby="sidebar-discard" aria-busy={busy}>
      <h2 id="sidebar-discard" className="sidebar-heading discard-heading">{heading}</h2>
      {count === 0 ? (
        <p className="sidebar-note" title={strings.discardEmptyHint}>{strings.discardEmpty}</p>
      ) : (
        <>
          <p className="discard-summary" aria-live="polite">{line}</p>
          <ul className="discard-list" aria-label={strings.discardListLabel}>
            {summary!.items.map((item) => (
              <li key={item.id} className="discard-row">
                <button type="button" className="discard-item" title={item.path}
                  aria-label={strings.discardShowItem(item.name, item.path)}
                  onClick={() => void revealQueued(item.id)}>
                  <Icon icon={item.is_dir ? Folder : File}
                    className={`kind-icon ${item.is_dir ? 'folder' : 'file'}`} />
                  <span className="discard-name">{item.path}</span>
                  <span className="discard-size">{formatBytes(item.size)}</span>
                </button>
                <button type="button" className="icon-btn discard-remove" disabled={busy}
                  aria-label={strings.discardRemoveItem(item.name)}
                  title={strings.removeFromDiscardPile}
                  onClick={() => useDiscardPile.getState().remove(item.id)}>
                  <Icon icon={X} size={14} />
                </button>
              </li>
            ))}
          </ul>
          <div className="discard-actions">
            <button type="button" className="btn" disabled={busy}
              onClick={() => useDiscardPile.getState().clear()}>{strings.discardClear}</button>
            <button type="button" className="btn danger" disabled={busy}
              onClick={() => useDiscardPile.getState().requestMoveAll()}>
              {busy ? strings.discardMoving : moveAll}
            </button>
          </div>
        </>
      )}
      {outcome && <Outcome />}
    </section>
  );
}

/** What the last batch did, with the failures one click away. */
function Outcome() {
  const outcome = useDiscardPile((s) => s.outcome)!;
  const text = outcome.error
    ? strings.discardBatchFailed(outcome.error)
    : strings.discardResult(
        formatInt(outcome.moved),
        formatBytes(outcome.bytes),
        formatInt(outcome.failed.length),
        outcome.moved === 1,
      );
  return (
    <div className="discard-outcome">
      <p>{text}</p>
      {outcome.failed.length > 0 && (
        <details>
          <summary>{strings.discardFailedList(formatInt(outcome.failed.length))}</summary>
          <ul aria-label={strings.discardFailedLabel}>
            {outcome.failed.map((item) => (
              <li key={item.id}>{strings.discardFailedItem(item.name, item.error)}</li>
            ))}
          </ul>
        </details>
      )}
      <button type="button" className="btn" onClick={() => useDiscardPile.getState().dismissOutcome()}>
        {strings.dismiss}
      </button>
    </div>
  );
}
