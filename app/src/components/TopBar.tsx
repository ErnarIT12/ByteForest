import { useAppStore } from '../store';
import { strings } from '../strings';
import { DrivePicker } from './DrivePicker';
import { FastScanButton } from './FastScanButton';
import { ScanButton } from './ScanButton';
import { ScanProgressText } from './ScanProgressText';
import { SettingsPopover } from './SettingsPopover';
import { TreeGlyph } from './TreeGlyph';

/** App bar: name, drive picker, Scan / Cancel and live progress. */
export function TopBar() {
  const scanning = useAppStore((s) => s.scanning);
  const isMac = useAppStore((s) => s.isMac);
  return (
    <header className="topbar">
      <div className="brand">
        <TreeGlyph />
        <span className="brand-name">{strings.appName}</span>
      </div>
      <div className="topbar-controls">
        <DrivePicker disabled={scanning} />
        <ScanButton />
        <FastScanButton />
        {scanning && <ScanProgressText />}
      </div>
      {!isMac && <SettingsPopover />}
    </header>
  );
}
