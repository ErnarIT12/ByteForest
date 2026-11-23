import { useEffect, useState } from 'react';
import { copyNodePath, nodeDetails, revealNodeInFileManager, type NodeDetails } from '../api';
import { formatBytes, formatFiletime } from '../format';
import { useAppStore } from '../store';

export function NodeDetailsPanel({ scanId, id }: { scanId: number; id: number }) {
  const [details, setDetails] = useState<NodeDetails | null>(null);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const isMac = useAppStore((s) => s.isMac);

  useEffect(() => {
    let alive = true;
    nodeDetails(scanId, id).then(
      (value) => { if (alive) setDetails(value); },
      (error: unknown) => { if (alive) setNotice(String(error)); },
    );
    return () => { alive = false; };
  }, [scanId, id]);

  const perform = async (action: (scan: number, node: number) => Promise<void>, success = '') => {
    setNotice('');
    setBusy(true);
    try {
      await action(scanId, id);
      setNotice(success);
    } catch (error) {
      setNotice(String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="node-details" aria-label="Selection details">
      {details && (
        <>
          <strong className="details-name" title={details.node.name}>{details.node.name}</strong>
          <div className="details-path" title={details.path}>{details.path}</div>
          <dl>
            <div><dt>Size</dt><dd>{formatBytes(details.node.size)}</dd></div>
            <div><dt>Allocated (estimate)</dt><dd>{formatBytes(details.node.alloc)}</dd></div>
            <div><dt>Modified</dt><dd>{formatFiletime(details.node.mtime) || '—'}</dd></div>
          </dl>
          {details.has_native_path ? (
            <div className="details-actions">
              <button type="button" className="btn" disabled={busy}
                onClick={() => void perform(copyNodePath, 'Path copied')}>Copy Path</button>
              <button type="button" className="btn" disabled={busy}
                onClick={() => void perform(revealNodeInFileManager)}>
                {isMac ? 'Show in Finder' : 'Show in Explorer'}
              </button>
            </div>
          ) : <p className="muted">This entry has no usable filesystem path.</p>}
        </>
      )}
      {notice && <p role="status">{notice}</p>}
    </section>
  );
}
