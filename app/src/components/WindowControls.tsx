import { useEffect, useState } from 'react';
import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useAppStore } from '../store';
import { Copy, Icon, Minus, Square, X } from './Icon';

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
        <Icon icon={Minus} size={14} />
      </button>
      <button type="button" disabled={!isTauri()} aria-label={toggleLabel} title={toggleLabel}
        onClick={() => void getCurrentWindow().toggleMaximize().catch(reportError)}>
        {maximized ? <Icon icon={Copy} size={13} /> : <Icon icon={Square} size={13} />}
      </button>
      <button type="button" disabled={!isTauri()} className="caption-close" aria-label="Close window" title="Close window"
        onClick={() => void getCurrentWindow().close().catch(reportError)}>
        <Icon icon={X} size={14} />
      </button>
    </div>
  );
}
