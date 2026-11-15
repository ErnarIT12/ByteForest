import { useEffect, useId, useRef, useState } from 'react';
import { getSettings, setSettings } from '../api';
import { strings } from '../strings';

/** Gear button with a small popover holding the app settings. */
export function SettingsPopover() {
  const [open, setOpen] = useState(false);
  const [always, setAlways] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const gear = useRef<HTMLButtonElement>(null);
  const checkbox = useRef<HTMLInputElement>(null);
  const panelId = useId();

  // Load the stored value each time the popover opens.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    getSettings().then(
      (s) => {
        if (cancelled) return;
        setAlways(s.always_elevated);
        setLoaded(true);
        setError(null);
      },
      () => {
        if (!cancelled) setError(strings.settingsLoadFailed);
      },
    );
    return () => {
      cancelled = true;
      setLoaded(false);
    };
  }, [open]);

  // Move focus into the popover once the checkbox is enabled.
  useEffect(() => {
    if (open && loaded) checkbox.current?.focus();
  }, [open, loaded]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false);
        gear.current?.focus();
      }
    };
    const onDown = (e: MouseEvent) => {
      if (root.current && !root.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('keydown', onKey);
    document.addEventListener('mousedown', onDown);
    return () => {
      document.removeEventListener('keydown', onKey);
      document.removeEventListener('mousedown', onDown);
    };
  }, [open]);

  const change = (value: boolean) => {
    const previous = always;
    setAlways(value);
    setError(null);
    setSettings({ always_elevated: value }).catch((e) => {
      setAlways(previous);
      setError(strings.settingsSaveFailed(String(e)));
    });
  };

  return (
    <div className="settings" ref={root}>
      <button
        ref={gear}
        type="button"
        className="icon-btn"
        aria-label={strings.settings}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        onClick={() => setOpen((o) => !o)}
      >
        <svg viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor">
          <circle cx="8" cy="8" r="2.2" />
          <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M12.6 3.4l-1.4 1.4M4.8 11.2l-1.4 1.4" />
        </svg>
      </button>
      {open && (
        <div className="popover" id={panelId} role="dialog" aria-label={strings.settings}>
          <label className="setting-row">
            <input
              ref={checkbox}
              type="checkbox"
              checked={always}
              disabled={!loaded}
              onChange={(e) => change(e.target.checked)}
            />
            <span>{strings.alwaysElevated}</span>
          </label>
          <p className="muted">{strings.alwaysElevatedHelp}</p>
          {error && (
            <p className="setting-error" role="alert">
              {error}
            </p>
          )}
        </div>
      )}
    </div>
  );
}
