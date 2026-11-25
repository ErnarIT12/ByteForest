import { WindowControls } from './WindowControls';
import { scanTargetFor } from '../scanTargets';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { FastScanButton } from './FastScanButton';
import { ScanButton } from './ScanButton';
import { ScanProgressText } from './ScanProgressText';
import { SettingsPopover } from './SettingsPopover';
import { SnapshotsMenu } from './SnapshotsMenu';
import { TreeGlyph } from './TreeGlyph';

/** App bar: name, current target with Scan / Fast scan, view switch, snapshots and settings. */
export function TopBar() {
  const hasResults = useAppStore((s) => s.root !== null);
  const scanning = useAppStore((s) => s.scanning);
  const isMac = useAppStore((s) => s.isMac);
  const target = useAppStore((s) => s.target);
  const source = useAppStore((s) => s.resultSource);
  const resultTarget = useAppStore((s) => s.resultTarget);
  const isSnapshot = source.kind === 'snapshot';
  // A snapshot view shows what the snapshot holds, not the sidebar selection.
  const shownTarget = isSnapshot && resultTarget !== null
    ? resultTarget
    : target === null ? null : scanTargetFor(target);
  return (
    <header className={isMac ? "topbar mac-caption" : "topbar"} data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <TreeGlyph />
        <span className="brand-name">{strings.appName}</span>
      </div>
      {hasResults && <div className="topbar-controls">
        {shownTarget && (
          <span className="topbar-target" title={shownTarget} aria-label={strings.topbarTargetLabel(shownTarget)}>
            {shownTarget}
          </span>
        )}
        {isSnapshot && <span className="badge snapshot-badge">{strings.snapshotBadge}</span>}
        <ScanButton />
        <FastScanButton />
        {scanning && <ScanProgressText />}
        <SnapshotsMenu />
      </div>}
      <div className="caption-space" data-tauri-drag-region />
      <SettingsPopover />
      {!isMac && <WindowControls />}
    </header>
  );
}
