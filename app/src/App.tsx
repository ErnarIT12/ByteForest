import { useEffect, useState } from 'react';
import { ElevationBar } from './components/ElevationBar';
import { EmptyState } from './components/EmptyState';
import { ErrorToast } from './components/ErrorToast';
import { FileActionsHost } from './components/FileActionsHost';
import { Explorer } from './components/Explorer';
import { OverviewView } from './components/OverviewView';
import { ScanAnnouncer } from './components/ScanAnnouncer';
import { Sidebar } from './components/Sidebar';
import { StatusBar } from './components/StatusBar';
import { TopBar } from './components/TopBar';
import { listenToScanEvents, loadEnvironment } from './scanController';
import { useAppStore } from './store';

/** Backspace goes up a level unless the user is typing into a control. */
function isTypingTarget(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))
  );
}

/** Window shell: full-height sidebar, minimal right-pane top bar, active view and status bar. */
export default function App() {
  const root = useAppStore((s) => s.root);
  const scanId = useAppStore((s) => s.resultScanId);
  const view = useAppStore((s) => s.resultView);
  const scanning = useAppStore((s) => s.scanning);
  const [split, setSplit] = useState(0.55);

  useEffect(() => {
    const signal = { cancelled: false };
    const ready = listenToScanEvents(signal);
    return () => {
      signal.cancelled = true;
      ready.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);
  useEffect(() => loadEnvironment(), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Backspace' && !isTypingTarget(e.target)) {
        e.preventDefault();
        useAppStore.getState().goUp();
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  const main =
    scanning && !root ? (
      <EmptyState />
    ) : view === 'overview' || !root || scanId === null ? (
      <OverviewView />
    ) : (
      <Explorer key={scanId} root={root} scanId={scanId} split={split} onSplitChange={setSplit} />
    );

  return (
    <div className="app">
      <TopBar />
      <ElevationBar />
      <Sidebar />
      {main}
      <StatusBar />
      <ErrorToast />
      <FileActionsHost />
      <ScanAnnouncer />
    </div>
  );
}
