import { useAppStore } from '../store';
import { strings } from '../strings';
import { DrivePicker } from './DrivePicker';
import { ScanButton } from './ScanButton';
import { ScanProgressText } from './ScanProgressText';
import { TreeGlyph } from './TreeGlyph';

/** App bar: name, drive picker, Scan / Cancel and live progress. */
export function TopBar() {
  const scanning = useAppStore((s) => s.scanning);
  return (
    <header className="topbar">
      <div className="brand">
        <TreeGlyph />
        <span className="brand-name">{strings.appName}</span>
      </div>
      <div className="topbar-controls">
        <DrivePicker disabled={scanning} />
        <ScanButton />
        {scanning && <ScanProgressText />}
      </div>
    </header>
  );
}
