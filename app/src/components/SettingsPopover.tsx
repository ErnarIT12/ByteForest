import { useEffect, useId, useRef, useState } from 'react';
import { getSettings, setSettings as saveSettings, type Settings } from '../api';
import { getAppearance, setAppearance, type Appearance } from '../hooks/useColorScheme';
import {
  AUTO_SNAPSHOT_CHOICES,
  addExclusion,
  mergeSettings,
  normalizeSettings,
  removeExclusion,
} from '../settingsModel';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { Icon, Settings as SettingsGlyph, X } from './Icon';

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
  const dialog = useRef<HTMLDialogElement>(null);
  const gear = useRef<HTMLButtonElement>(null);
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

  useEffect(() => {
    if (open) dialog.current?.showModal();
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
    <div className="settings">
      <button
        ref={gear}
        type="button"
        className="sidebar-item"
        title={strings.settings}
        aria-label={strings.settings}
        aria-haspopup="dialog"
        aria-expanded={open}
        aria-controls={open ? panelId : undefined}
        onClick={() => setOpen((o) => !o)}
      >
        <Icon icon={SettingsGlyph} /><span className="sidebar-text">{strings.settings}</span>
      </button>
      {open && (
        <dialog className="settings-dialog" ref={dialog} id={panelId} aria-label={strings.settings}
          onClose={() => { setOpen(false); gear.current?.focus(); }}>
          <button className="icon-btn dialog-close" type="button" aria-label="Close settings"
            onClick={() => dialog.current?.close()}><Icon icon={X} /></button>
          <h2>Settings</h2>
          <label className="appearance-setting">Appearance
            <select defaultValue={getAppearance()}
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
          <label className="appearance-setting">{strings.autoSnapshotsLabel}
            <select
              value={settings?.auto_snapshot_keep ?? ''}
              disabled={!settings}
              aria-describedby={`${panelId}-auto-help`}
              onChange={(e) => change({ auto_snapshot_keep: Number(e.target.value) })}
            >
              {settings && !(AUTO_SNAPSHOT_CHOICES as readonly number[]).includes(settings.auto_snapshot_keep) && (
                <option value={settings.auto_snapshot_keep}>{settings.auto_snapshot_keep}</option>
              )}
              {AUTO_SNAPSHOT_CHOICES.map((n) => (
                <option key={n} value={n}>{n === 0 ? strings.autoSnapshotsOff : n}</option>
              ))}
            </select>
          </label>
          <p className="muted" id={`${panelId}-auto-help`}>{strings.autoSnapshotsHelp}</p>
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
                      <Icon icon={X} />
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
        </dialog>
      )}
    </div>
  );
}
