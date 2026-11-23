import { useAppStore } from '../store';
import { strings } from '../strings';
import { ScanButton } from './ScanButton';
import { DrivePicker } from './DrivePicker';
import { ScanProgressText } from './ScanProgressText';
import { TreeGlyph } from './TreeGlyph';

/** Centered invitation before the first result, or the first scan's progress. */
export function EmptyState() {
  const scanning = useAppStore((s) => s.scanning);
  const isMac = useAppStore((s) => s.isMac);
  const scanTarget = useAppStore((s) => s.scanTarget);

  return (
    <section className="empty">
      <div className="empty-card">
        <TreeGlyph size={48} />
        {scanning ? (
          <>
            <h1>{strings.scanningTitle(scanTarget ?? '')}</h1>
            <ScanProgressText />
            <div className="empty-actions">
              <ScanButton />
            </div>
          </>
        ) : (
          <>
            <h1>{strings.emptyTitle}</h1>
            <p>{isMac ? "Choose a volume or folder to see what takes up space." : strings.emptyBody}</p>
            <div className="empty-actions">
              <DrivePicker disabled={false} />
              <ScanButton />
            </div>
          </>
        )}
      </div>
    </section>
  );
}
