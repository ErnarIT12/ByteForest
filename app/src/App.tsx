import { useEffect, useState } from 'react';
import { ElevationBar } from './components/ElevationBar';
import { EmptyState } from './components/EmptyState';
import { ErrorToast } from './components/ErrorToast';
import { FileActionsHost } from './components/FileActionsHost';
import { Explorer } from './components/Explorer';
import { ScanAnnouncer } from './components/ScanAnnouncer';
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

/** Window shell: top bar, the Explorer (or the empty state) and the status bar. */
export default function App() {
  const root = useAppStore((s) => s.root);
  const scanId = useAppStore((s) => s.resultScanId);
  const [split, setSplit] = useState(0.74);

  useEffect(() => {
    const ready = listenToScanEvents();
    return () => {
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

  return (
    <div className="app">
      <TopBar />
      <ElevationBar />
      {root && scanId !== null ? <Explorer key={scanId} root={root} scanId={scanId} split={split} onSplitChange={setSplit} /> : <EmptyState />}
      <StatusBar />
      <ErrorToast />
      <FileActionsHost />
      <ScanAnnouncer />
    </div>
  );
}
