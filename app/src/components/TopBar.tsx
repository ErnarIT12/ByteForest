import { WindowControls } from './WindowControls';
import { scanTargetFor } from '../scanTargets';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { FastScanButton } from './FastScanButton';
import { ScanButton } from './ScanButton';
import { ScanProgressText } from './ScanProgressText';

/** Minimal right-pane top bar: clean "Overview" title on Overview, scan target & controls on scan views. */
export function TopBar() {
  const hasResults = useAppStore((s) => s.root !== null);
  const view = useAppStore((s) => s.resultView);
  const scanning = useAppStore((s) => s.scanning);
  const isMac = useAppStore((s) => s.isMac);
  const target = useAppStore((s) => s.target);
  const source = useAppStore((s) => s.resultSource);
  const resultTarget = useAppStore((s) => s.resultTarget);
  const lastScan = useAppStore((s) => s.recentScans.find((entry) => entry.target === s.resultTarget));
  const isSnapshot = source.kind === 'snapshot';
  const onOverview = !scanning && (!hasResults || view === 'overview');
  // A snapshot view shows what the snapshot holds, not the sidebar selection.
  const shownTarget = isSnapshot && resultTarget !== null
    ? resultTarget
    : target === null ? null : scanTargetFor(target);
  return (
    <header className="topbar" data-tauri-drag-region>
      {onOverview ? (
        <span className="topbar-title" data-tauri-drag-region>{strings.overviewTitle}</span>
      ) : hasResults ? (
        <div className="topbar-controls">
          {shownTarget && (
            <span className="topbar-target" title={shownTarget} aria-label={strings.topbarTargetLabel(shownTarget)}>
              {shownTarget}
            </span>
          )}
          {isSnapshot && <span className="badge snapshot-badge">{strings.snapshotBadge}</span>}
          {!isSnapshot && !scanning && lastScan && shownTarget === resultTarget && (
            <time className="last-scan" dateTime={new Date(lastScan.finishedAt).toISOString()}
              title={new Date(lastScan.finishedAt).toLocaleString()}>
              Scanned {new Date(lastScan.finishedAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
            </time>
          )}
          <ScanButton compact />
          <FastScanButton />
          {scanning && <ScanProgressText />}
        </div>
      ) : null}
      <div className="caption-space" data-tauri-drag-region />
      {!isMac && <WindowControls />}
    </header>
  );
}
