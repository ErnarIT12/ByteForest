import { useEffect } from 'react';
import { ElevationBar } from './components/ElevationBar';
import { EmptyState } from './components/EmptyState';
import { ErrorToast } from './components/ErrorToast';
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
  const generation = useAppStore((s) => s.generation);

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
      {root ? <Explorer root={root} generation={generation} /> : <EmptyState />}
      <StatusBar />
      <ErrorToast />
      <ScanAnnouncer />
    </div>
  );
}
