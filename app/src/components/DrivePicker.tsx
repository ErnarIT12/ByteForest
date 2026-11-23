import { pickFolder } from '../api';
import { formatBytes } from '../format';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Drive `<select>`: "C:  477 GB · NTFS". */
export function DrivePicker({ disabled, expanded = false }: { disabled: boolean; expanded?: boolean }) {
  const drives = useAppStore((s) => s.drives);
  const target = useAppStore((s) => s.target);
  const setTarget = useAppStore((s) => s.setTarget);
  const isMac = useAppStore((s) => s.isMac);

  const chooseFolder = () => {
    void pickFolder().then(
      (folder) => { if (folder) setTarget(folder); },
      (error: unknown) => useAppStore.setState({ error: String(error), errorTitle: 'Unable to choose a folder' }),
    );
  };

  if (drives === null) {
    return <span className="muted">{strings.loadingDrives}</span>;
  }
  if (!isMac && drives.length === 0) {
    return <span className="muted">{strings.noDrives}</span>;
  }
  const customFolder = isMac && target && !drives.some((drive) => drive.path === target);
  if (expanded) {
    const locations = drives.map((drive) => ({
      value: drive.path ?? drive.letter ?? '',
      name: drive.path ? drive.label : [drive.letter + ':', drive.label].filter(Boolean).join('  '),
      description: drive.total > 0
        ? formatBytes(drive.free) + ' free of ' + formatBytes(drive.total)
        : drive.path || drive.fs,
      used: drive.total > 0 ? Math.max(0, Math.min(1, 1 - drive.free / drive.total)) : null,
    }));
    if (customFolder) locations.push({ value: target, name: 'Selected folder', description: target, used: null });
    return (
      <fieldset className="locations" disabled={disabled}>
        <legend className="sr-only">Scan location</legend>
        {locations.map(({ value, name, description, used }) => (
          <label className="location-row" key={value}>
            <input type="radio" name="scan-location" value={value}
              checked={target === value} onChange={() => setTarget(value)} />
            <span className="location-info">
              <strong>{name}</strong>
              <span>{description}</span>
            </span>
            {used !== null && (
              <span className="location-meter" aria-hidden="true">
                <span style={{ width: `${100 * used}%` }} />
              </span>
            )}
          </label>
        ))}
        {isMac && <button type="button" className="btn choose-folder" onClick={chooseFolder}>Choose folder…</button>}
      </fieldset>
    );
  }
  return (
    <select
      className="drive-select"
      aria-label={isMac ? "Scan location" : strings.driveLabel}
      value={target ?? ''}
      disabled={disabled}
      onChange={(e) => {
        if (e.target.value === 'choose-folder') {
          e.target.value = target ?? '';
          chooseFolder();
        }
        else setTarget(e.target.value);
      }}
    >
      {isMac && target === null && <option value="" disabled>Select a location</option>}
      {drives.map((drive) => (
        <option key={drive.path ?? drive.letter} value={drive.path ?? drive.letter}
          title={drive.path ?? drive.label}>
          {drive.path ? drive.label : drive.letter + ': ' + formatBytes(drive.total, 0) + ' · ' + drive.fs}
        </option>
      ))}
      {customFolder && <option value={target}>{target}</option>}
      {isMac && <option value="choose-folder">Choose Folder…</option>}
    </select>
  );
}
