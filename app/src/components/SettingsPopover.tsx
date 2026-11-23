import { useEffect, useId, useRef, useState } from 'react';
import { getSettings, setSettings as saveSettings, type Settings } from '../api';
import { getAppearance, setAppearance, type Appearance } from '../hooks/useColorScheme';
import {
  addExclusion,
  mergeSettings,
  normalizeSettings,
  removeExclusion,
} from '../settingsModel';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Gear button with a small popover holding the app settings. */
export function SettingsPopover() {
  const [open, setOpen] = useState(false);
  // The settings as last loaded or saved; null until loaded. `set_settings` replaces the whole
  // file, so every save sends this whole object with the change merged in.
  const [settings, setSettingsState] = useState<Settings | null>(null);
  const settingsRef = useRef<Settings | null>(null);
  const setCurrent = (value: Settings | null) => {
    settingsRef.current = value;
    setSettingsState(value);
  };
  const [draft, setDraft] = useState('');
  const [draftError, setDraftError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const gear = useRef<HTMLButtonElement>(null);
  const appearance = useRef<HTMLSelectElement>(null);
  const isMac = useAppStore((s) => s.isMac);
  const panelId = useId();

  // Load the stored value each time the popover opens.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    getSettings().then(
      (s) => {
        if (cancelled) return;
        setCurrent(normalizeSettings(s));
        setError(null);
      },
      () => {
        if (!cancelled) setError(strings.settingsLoadFailed);
      },
    );
    return () => {
      cancelled = true;
      setCurrent(null);
      setDraft('');
      setDraftError(null);
    };
  }, [open]);

  // Put keyboard focus on the appearance control when the panel opens.
  useEffect(() => {
    if (open) appearance.current?.focus();
  }, [open]);

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

  /** Saves the settings with `patch` applied; the other fields are sent unchanged. */
  const change = (patch: Partial<Settings>) => {
    const previous = settingsRef.current;
    if (!previous) return;
    const next = mergeSettings(previous, patch);
    setCurrent(next);
    setError(null);
    saveSettings(next).catch((e) => {
      // A newer change has taken over; do not undo it.
      if (settingsRef.current === next) setCurrent(previous);
      setError(strings.settingsSaveFailed(String(e)));
    });
  };

  const exclusions = settings?.exclusions ?? [];

  const addDraft = () => {
    if (!settings) return;
    const result = addExclusion(settings.exclusions, draft);
    if (!result.ok) {
      if (result.reason === 'tooLong') setDraftError(strings.exclusionTooLong);
      else if (result.reason === 'tooMany') setDraftError(strings.exclusionTooMany);
      return;
    }
    setDraftError(null);
    setDraft('');
    if (result.list.length !== settings.exclusions.length) change({ exclusions: result.list });
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
          <h2>Settings</h2>
          <label className="appearance-setting">Appearance
            <select ref={appearance} defaultValue={getAppearance()}
              onChange={(e) => setAppearance(e.target.value as Appearance)}>
              <option value="dark">Graphite</option>
              <option value="light">Silver</option>
              <option value="system">System</option>
            </select>
          </label>
          {!isMac && <><label className="setting-row">
            <input
              type="checkbox"
              checked={settings?.always_elevated ?? false}
              disabled={!settings}
              onChange={(e) => change({ always_elevated: e.target.checked })}
            />
            <span>{strings.alwaysElevated}</span>
          </label>
          <p className="muted">{strings.alwaysElevatedHelp}</p></>}
          <section className="exclusions" aria-labelledby={`${panelId}-excl`}>
            <h3 id={`${panelId}-excl`}>{strings.exclusionsTitle}</h3>
            {settings && exclusions.length === 0 && <p className="muted">{strings.exclusionsEmpty}</p>}
            {exclusions.length > 0 && (
              <ul className="exclusion-list">
                {exclusions.map((pattern) => (
                  <li key={pattern}>
                    <code title={pattern}>{pattern}</code>
                    <button
                      type="button"
                      className="icon-btn"
                      aria-label={strings.exclusionRemove(pattern)}
                      title={strings.exclusionRemove(pattern)}
                      onClick={() => change({ exclusions: removeExclusion(exclusions, pattern) })}
                    >
                      <svg viewBox="0 0 16 16" aria-hidden="true" fill="none" stroke="currentColor">
                        <path d="M4 4l8 8M12 4l-8 8" />
                      </svg>
                    </button>
                  </li>
                ))}
              </ul>
            )}
            <form
              className="exclusion-add"
              onSubmit={(e) => {
                e.preventDefault();
                addDraft();
              }}
            >
              <input
                type="text"
                value={draft}
                disabled={!settings}
                maxLength={400}
                placeholder={strings.exclusionPlaceholder}
                aria-label={strings.exclusionInputLabel}
                aria-describedby={`${panelId}-excl-help`}
                spellCheck={false}
                onChange={(e) => {
                  setDraft(e.target.value);
                  setDraftError(null);
                }}
              />
              <button type="submit" className="btn" disabled={!settings || draft.trim() === ''}>
                {strings.exclusionAdd}
              </button>
            </form>
            {draftError && (
              <p className="setting-error" role="alert">
                {draftError}
              </p>
            )}
            <p className="muted" id={`${panelId}-excl-help`}>
              {strings.exclusionsHelp} {strings.exclusionsNextScan}
            </p>
          </section>
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
