import { useAppStore } from '../store';
import { formatBytes } from '../format';
import { strings } from '../strings';
import { ScanButton } from './ScanButton';
import { DrivePicker } from './DrivePicker';
import { ScanProgressText } from './ScanProgressText';
import { TreeGlyph } from './TreeGlyph';
import { FastScanButton } from './FastScanButton';

function VolumeUsage() {
  const drive = useAppStore((s) => s.drives?.find((d) => (d.path ?? d.letter) === s.target));
  if (!drive || drive.total <= 0) return null;
  const used = Math.max(0, drive.total - drive.free);
  const percent = Math.min(100, Math.round(100 * used / drive.total));
  return (
    <div className="volume-summary">
      <div className="volume-ring" style={{ backgroundImage: `conic-gradient(var(--accent) ${percent}%, var(--bar-track) 0)` }}>
        <span><strong>{percent}%</strong>used</span>
      </div>
      <div className="volume-figures">
        <div><strong>{formatBytes(used)}</strong><span>Used</span></div>
        <div><strong>{formatBytes(drive.free)}</strong><span>Available</span></div>
      </div>
    </div>
  );
}

/** Centered invitation before the first result, or the first scan's progress. */
export function EmptyState() {
  const scanning = useAppStore((s) => s.scanning);
  const isMac = useAppStore((s) => s.isMac);
  const scanTarget = useAppStore((s) => s.scanTarget);

  return (
    <section className="empty">
      <div className="empty-card">
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
            <div className="location-intro">
              <TreeGlyph size={108} />
              <h1>{strings.emptyTitle}</h1>
              <p>{isMac ? "Choose a volume or folder to see what takes up space." : strings.emptyBody}</p>
            </div>
            <div className="location-choices">
              <h2>Scan location</h2>
              <DrivePicker disabled={false} expanded />
              <VolumeUsage />
              <div className="empty-actions">
                <ScanButton />
                <FastScanButton />
              </div>
            </div>
          </>
        )}
      </div>
    </section>
  );
}
