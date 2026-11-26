import { FolderOpen, HardDrive } from 'lucide-react';
import { Icon } from './Icon';
import type { VolumeSummary } from '../api';
import { useAppStore } from '../store';
import { chooseFolderAndScan } from '../scanController';
import { formatBytes } from '../format';
import { scanTargetFor } from '../scanTargets';
import { strings } from '../strings';
import { ScanButton } from './ScanButton';
import { ScanProgressText } from './ScanProgressText';
import { TreeGlyph } from './TreeGlyph';
import { FastScanButton } from './FastScanButton';

export function VolumeUsage({ drive }: { drive: VolumeSummary }) {
  const free = Math.max(0, Math.min(drive.total, drive.free));
  const used = drive.total - free;
  const percent = Math.round(100 * used / drive.total);
  return (
    <div className="volume-summary">
      <div className="volume-ring">
        <svg viewBox="0 0 100 100" aria-hidden="true">
          <circle className="volume-track" cx="50" cy="50" r="43" />
          <circle className="volume-fill" cx="50" cy="50" r="43" pathLength="100"
            strokeDasharray="100" strokeDashoffset={100 - percent} />
        </svg>
        <span><strong>{percent}%</strong> used</span>
      </div>
      <div className="volume-figures">
        <span>{strings.volumeAvailable}</span>
        <strong>{formatBytes(free)}</strong>
        <small>{formatBytes(used)} used of {formatBytes(drive.total)}</small>
      </div>
    </div>
  );
}

/** Centered invitation before the first result, or the first scan's progress. */
export function EmptyState() {
  const scanning = useAppStore((s) => s.scanning);
  const scanTarget = useAppStore((s) => s.scanTarget);
  const target = useAppStore((s) => s.target);
  const shownTarget = target === null ? null : scanTargetFor(target);

  const drive = useAppStore((s) => s.drives?.find((d) => (d.path ?? d.letter) === s.target));
  const heading = drive?.label || (drive?.letter ? `Drive ${shownTarget}` : shownTarget);
  return (
    <section className="empty">
      <div className="empty-card" key={scanning ? 'scanning' : 'ready'}>
        {scanning ? (
          <>
            <TreeGlyph size={100} />
            <h1>{strings.scanningTitle(scanTarget ?? '')}</h1>
            <ScanProgressText />
            <div className="empty-actions">
              <ScanButton />
            </div>
          </>
        ) : (
          <>
            <div className="scan-location">
              <Icon icon={drive?.total ? HardDrive : FolderOpen} size={24} />
              <h1 title={shownTarget ?? undefined}>{heading ?? strings.emptyTitle}</h1>
            </div>
            {drive && drive.total > 0 ? <VolumeUsage drive={drive} /> :
              <p>{shownTarget ? strings.emptyBody : strings.emptyChooseLocation}</p>}
            {drive?.label && shownTarget !== drive.label && <p className="location-path">{shownTarget}</p>}
            <div className="empty-actions">
              {shownTarget !== null && <ScanButton label={drive?.total ? 'Scan drive' : 'Scan folder'} />}
              <button type="button" className={shownTarget === null ? 'btn primary' : 'btn'} onClick={chooseFolderAndScan}>
                <Icon icon={FolderOpen} />{strings.chooseFolder}
              </button>
            </div>
            <FastScanButton subtle />
          </>
        )}
      </div>
    </section>
  );
}
