import { useEffect, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useAppStore } from '../store';

function reportError(error: unknown) {
  useAppStore.setState({ errorTitle: 'Window control unavailable', error: String(error) });
}

export function WindowControls() {
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    const win = getCurrentWindow();
    let active = true;
    const sync = () => win.isMaximized().then((value) => {
      if (active) setMaximized(value);
    });
    void sync().catch(reportError);
    const stop = win.onResized(() => { void sync().catch(reportError); });
    return () => {
      active = false;
      void stop.then((unlisten) => unlisten()).catch(reportError);
    };
  }, []);

  const toggleLabel = maximized ? 'Restore window' : 'Maximize window';
  return (
    <div className="caption-buttons" role="group" aria-label="Window controls">
      <button type="button" disabled={!isTauri()} aria-label="Minimize window" title="Minimize window"
        onClick={() => void getCurrentWindow().minimize().catch(reportError)}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="M2 8.5h12" /></svg>
      </button>
      <button type="button" disabled={!isTauri()} aria-label={toggleLabel} title={toggleLabel}
        onClick={() => void getCurrentWindow().toggleMaximize().catch(reportError)}>
        <svg viewBox="0 0 16 16" aria-hidden="true">
          {maximized ? <path d="M5 4V2h9v9h-2 M2 5h9v9H2z" /> : <rect x="3" y="3" width="10" height="10" />}
        </svg>
      </button>
      <button type="button" disabled={!isTauri()} className="caption-close" aria-label="Close window" title="Close window"
        onClick={() => void getCurrentWindow().close().catch(reportError)}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3 3 10 10 M13 3 3 13" /></svg>
      </button>
    </div>
  );
}
