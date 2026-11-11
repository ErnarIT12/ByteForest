import { formatBytes } from '../format';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Drive `<select>`: "C:  477 GB · NTFS". */
export function DrivePicker({ disabled }: { disabled: boolean }) {
  const drives = useAppStore((s) => s.drives);
  const target = useAppStore((s) => s.target);
  const setTarget = useAppStore((s) => s.setTarget);

  if (drives === null) {
    return <span className="muted">{strings.loadingDrives}</span>;
  }
  if (drives.length === 0) {
    return <span className="muted">{strings.noDrives}</span>;
  }
  return (
    <select
      className="drive-select"
      aria-label={strings.driveLabel}
      value={target ?? ''}
      disabled={disabled}
      onChange={(e) => setTarget(e.target.value)}
    >
      {drives.map((d) => (
        <option key={d.letter} value={d.letter} title={d.label || undefined}>
          {`${d.letter}: ${formatBytes(d.total, 0)} · ${d.fs}`}
        </option>
      ))}
    </select>
  );
}
