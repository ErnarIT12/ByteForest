import { useEffect, useState } from 'react';
import { copyNodePath, exportReport, nodeDetails, revealNodeInFileManager, type NodeDetails } from '../api';
import { deltaCell } from '../diffView';
import { formatBytes, formatDelta, formatFiletime, formatInt, formatScanShare } from '../format';
import { useFileActions } from '../fileActions';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { FileTypeBreakdown } from './FileTypeBreakdown';

export function NodeDetailsPanel({ scanId, id, scanSize }: { scanId: number; id: number; scanSize: number }) {
  const [details, setDetails] = useState<NodeDetails | null>(null);
  const [notice, setNotice] = useState('');
  const [busy, setBusy] = useState(false);
  const isMac = useAppStore((s) => s.isMac);
  const rootId = useAppStore((s) => s.rootId);
  const comparing = useAppStore((s) => s.baseline !== null);

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

  const exportFolderReport = async () => {
    setNotice('');
    setBusy(true);
    try {
      const path = await exportReport(scanId, id);
      if (path) setNotice(strings.reportExported(path));
    } catch (error) {
      setNotice(`${strings.reportExportFailed}: ${String(error)}`);
    } finally {
      setBusy(false);
    }
  };

  const diff = details?.node.diff;
  const change = deltaCell(diff);

  return (
    <section className="node-details" aria-label="Selection details">
      {details && (
        <>
          <div className="details-heading">
            <strong className="details-name" title={details.node.name}>{details.node.name}</strong>
            <strong className="selection-size">{formatBytes(details.node.size)}</strong>
          </div>
          <div className="details-path" title={details.path}>{details.path}</div>
          <div className="share-meter" aria-hidden="true"><span style={{
            width: scanSize > 0 ? Math.min(100, details.node.size * 100 / scanSize) + '%' : '0%',
          }} /></div>
          <dl>
            <div><dt>Kind</dt><dd>{details.node.is_dir ? 'Folder' : 'File'}</dd></div>
            <div><dt>Of scan</dt><dd>{formatScanShare(details.node.size, scanSize)}</dd></div>
            <div><dt>Allocated (estimate)</dt><dd>{formatBytes(details.node.alloc)}</dd></div>
            <div><dt>Modified</dt><dd>{formatFiletime(details.node.mtime) || '—'}</dd></div>
            {comparing && change && diff && (
              <div><dt>{strings.sinceSnapshot}</dt><dd>
                <span className={`delta ${change.tone}`}>
                  {diff.status === 'added' ? `${change.text} ${formatDelta(diff.delta)}` : change.text}
                </span>
              </dd></div>
            )}
            {comparing && diff?.removed && (
              <div><dt>{strings.goneSinceSnapshot}</dt>
                <dd>{strings.goneSummary(formatInt(diff.removed.count), formatBytes(diff.removed.bytes))}</dd></div>
            )}
          </dl>
          {details.node.is_dir && (
            <FileTypeBreakdown scanId={scanId} id={id} name={details.node.name} />
          )}
          {details.node.is_dir && (
            <div className="details-actions">
              <button type="button" className="btn" disabled={busy}
                onClick={() => void exportFolderReport()}>{strings.exportReport}</button>
            </div>
          )}
          {details.has_native_path ? (
            <div className="details-actions">
              <button type="button" className="btn" disabled={busy}
                onClick={() => void perform(copyNodePath, strings.pathCopied)}>{strings.copyPath}</button>
              <button type="button" className="btn" disabled={busy}
                onClick={() => void perform(revealNodeInFileManager)}>
                {isMac ? strings.showInFinder : strings.showInExplorer}
              </button>
              {id !== rootId && (
                <button type="button" className="btn danger" disabled={busy}
                  onClick={() => void useFileActions.getState().requestTrash(scanId, id)}>
                  {isMac ? strings.moveToTrash : strings.moveToRecycleBin}
                </button>
              )}
            </div>
          ) : <p className="muted">{strings.noNativePath}</p>}
        </>
      )}
      {notice && <p role="status">{notice}</p>}
    </section>
  );
}
