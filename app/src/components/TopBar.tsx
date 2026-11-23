import { WindowControls } from './WindowControls';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { DrivePicker } from './DrivePicker';
import { FastScanButton } from './FastScanButton';
import { ScanButton } from './ScanButton';
import { ScanProgressText } from './ScanProgressText';
import { SettingsPopover } from './SettingsPopover';
import { SnapshotsMenu } from './SnapshotsMenu';
import { TreeGlyph } from './TreeGlyph';

/** App bar: name, drive picker, Scan / Cancel and live progress. */
export function TopBar() {
  const hasResults = useAppStore((s) => s.root !== null);
  const scanning = useAppStore((s) => s.scanning);
  const isMac = useAppStore((s) => s.isMac);
  return (
    <header className={isMac ? "topbar mac-caption" : "topbar"} data-tauri-drag-region>
      <div className="brand" data-tauri-drag-region>
        <TreeGlyph />
        <span className="brand-name">{strings.appName}</span>
      </div>
      {hasResults && <div className="topbar-controls">
        <DrivePicker disabled={scanning} />
        <ScanButton />
        <FastScanButton />
        <SnapshotsMenu />
        {scanning && <ScanProgressText />}
      </div>}
      <div className="caption-space" data-tauri-drag-region />
      <SettingsPopover />
      {!isMac && <WindowControls />}
    </header>
  );
}
