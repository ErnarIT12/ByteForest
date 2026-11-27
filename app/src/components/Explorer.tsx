import { useEffect, useRef, useState } from 'react';
import { setTreemapColorMode, useTreemapColorMode } from '../hooks/useTreemapColorMode';
import { locateNode, type NodeView } from '../api';
import { formatBytes, formatInt } from '../format';
import { useAppStore } from '../store';
import { Breadcrumbs } from './Breadcrumbs';
import { ChangesList } from './ChangesList';
import { CompareBanner } from './CompareBanner';
import { SnapshotBanner } from './SnapshotBanner';
import { RecentScans } from './RecentScans';
import { SnapshotsMenu } from './SnapshotsMenu';
import { CleanupQueue } from './DiscardPileDock';
import { DuplicatesView } from './DuplicatesView';
import { FileSearch } from './FileSearch';
import { NodeDetailsPanel } from './NodeDetailsPanel';
import { strings } from '../strings';
import { MapViewToggle } from './MapViewToggle';
import { Splitter } from './Splitter';
import { TreemapCanvas } from './TreemapCanvas';
import { DiffLegend, TreemapLegend } from './TreemapLegend';
import { SunburstCanvas } from './SunburstCanvas';
import { TreePanel } from './TreePanel';

/** The Explorer screen: disk map and file browser. */
export function Explorer({ root, scanId, split, onSplitChange }: {
  root: NodeView;
  scanId: number;
  split: number;
  onSplitChange: (value: number) => void;
}) {
  const [treeShare, setTreeShare] = useState(0.6);
  const [mapOnly, setMapOnly] = useState(false);
  const view = useAppStore((s) => s.resultView);
  const search = view === 'files';
  const [seenView, setSeenView] = useState(view);
  const [changesMode, setChangesMode] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (seenView !== view) {
    setSeenView(view);
    setOpening(false);
    setError(null);
  }
  const request = useRef(0);
  const treeButton = useRef<HTMLButtonElement>(null);
  const focusId = useAppStore((s) => s.focusId);
  const selected = useAppStore((s) => s.selectedId);
  const generation = useAppStore((s) => s.generation);
  const comparing = useAppStore((s) => s.baseline !== null);
  const mapView = useAppStore((s) => s.mapView);
  // The Changes view only exists while comparing.
  const changes = changesMode && comparing;
  const colorMode = useTreemapColorMode();
  const presetSeq = useAppStore((s) => s.searchPreset?.seq ?? 0);
  // A new search preset (from the file-type breakdown) opens the search pane. Adjusting state
  // while rendering avoids an extra effect pass.
  const [seenPreset, setSeenPreset] = useState(presetSeq);
  if (presetSeq !== seenPreset) {
    setSeenPreset(presetSeq);
    if (presetSeq > 0) {
      setOpening(false);
      setError(null);
      setChangesMode(false);
      setMapOnly(false);
    }
  }

  // A reveal that comes from outside the tree (the Discard Pile) brings the tree back.
  const revealSeq = useAppStore((s) => (s.reveal?.showTree ? s.reveal.seq : 0));
  const [seenReveal, setSeenReveal] = useState(revealSeq);
  if (revealSeq !== seenReveal) {
    setSeenReveal(revealSeq);
    if (revealSeq > 0) {
      setOpening(false);
      setError(null);
      setChangesMode(false);
      setMapOnly(false);
    }
  }

  // Drops a pending "locate file" request so it cannot switch the pane back.
  useEffect(() => {
    request.current += 1;
  }, [presetSeq, view]);

  const showSearch = (value: boolean) => {
    request.current += 1;
    setOpening(false);
    setError(null);
    useAppStore.setState({ resultView: value ? 'files' : 'map' });
    setChangesMode(false);
  };

  const showChanges = () => {
    request.current += 1;
    setOpening(false);
    setError(null);
    useAppStore.setState({ resultView: 'map' });
    setChangesMode(true);
  };

  const openResult = async (id: number) => {
    const sequence = ++request.current;
    const isCurrent = () =>
      sequence === request.current && useAppStore.getState().resultScanId === scanId;
    setOpening(true);
    setError(null);
    try {
      const steps = await locateNode(scanId, id);
      if (!isCurrent()) return;
      useAppStore.getState().revealSearchResult(scanId, steps);
      setChangesMode(false);
      setMapOnly(false);
      requestAnimationFrame(() => treeButton.current?.focus());
    } catch (failure) {
      if (isCurrent()) setError(String(failure));
    } finally {
      if (isCurrent()) setOpening(false);
    }
  };
  if (view === 'dupes') return (
    <DuplicatesView scanId={scanId} root={root} onOpen={(id) => void openResult(id)}
      opening={opening} navigationError={error} />
  );
  if (view === 'history') return (
    <main className="explorer cleanup-view">
      <header className="scan-heading"><h1>Scan history</h1><SnapshotsMenu /></header>
      <section className="cleanup-content history-content">
        <RecentScans rail={false} detailed />
        {comparing && <>
          <CompareBanner />
          <ChangesList scanId={scanId} active opening={opening} navigationError={error}
            onOpen={(id) => void openResult(id)} />
        </>}
      </section>
    </main>
  );
  if (view === 'cleanup') return (
    <main className="explorer cleanup-view">
      <header className="scan-heading"><h1>Cleanup</h1></header>
      <section className="cleanup-content">
        <h2>Cache folders</h2>
        <p>Find common cache folder names in this scan. Review their paths before choosing what to move.</p>
        <button type="button" className="btn" onClick={() => useAppStore.getState().openSearch({
          under: { id: root.id, name: root.name }, caches: true,
        })}>Find cache folders</button>
        <CleanupQueue />
      </section>
    </main>
  );
  return (
    <main
      className={search ? "explorer files-expanded" : mapOnly ? "explorer map-expanded" : "explorer"}
      // Flexible tracks share the width split : (1 - split) whatever the content; the
      // minimums keep both panes usable in a narrow window.
      style={{ gridTemplateColumns: search || mapOnly ? 'minmax(0, 1fr)' : `minmax(300px, ${split}fr) 8px minmax(280px, ${1 - split}fr)`,
        gridTemplateRows: search || mapOnly ? undefined : `auto minmax(110px, ${treeShare}fr) 8px minmax(100px, ${1 - treeShare}fr)` }}
    >
      <header className="scan-heading">
        <h1 title={root.name}>{search ? 'Largest files' : root.name}</h1>
        <dl className="scan-totals">
          <div><dt>Scanned</dt><dd>{formatBytes(root.size)}</dd></div>
          <div><dt>Files</dt><dd>{formatInt(root.files)}</dd></div>
          <div><dt>Folders</dt><dd>{formatInt(root.dirs)}</dd></div>
        </dl>
      </header>
      {!search && <section className="map-panel">
        <div className="map-toolbar">
          <MapViewToggle />
          <Breadcrumbs />
          <div className="segmented" role="group" aria-label={strings.colorModeLabel}>
            <button type="button" aria-pressed={colorMode === 'depth'}
              onClick={() => setTreemapColorMode('depth')}>{strings.colorModeFolders}</button>
            <button type="button" aria-pressed={colorMode === 'category'}
              onClick={() => setTreemapColorMode('category')}>{strings.colorModeTypes}</button>
          </div>
          <button type="button" className="map-mode" aria-controls="result-browser"
            aria-expanded={!mapOnly} onClick={() => setMapOnly(!mapOnly)}>
            {mapOnly ? 'Show file list' : 'Expand map'}
          </button>
        </div>
        {colorMode === 'category' && <TreemapLegend />}
        {comparing && <DiffLegend />}
        {mapView === 'sunburst' ? <SunburstCanvas /> : <TreemapCanvas />}
      </section>}
      {!search && !mapOnly && <Splitter value={split} min={0.45} max={0.82} onChange={onSplitChange} />}
      <section id="result-browser" hidden={mapOnly && !search} className="explorer-sidebar" aria-label="Scan results">
        <SnapshotBanner />
        <CompareBanner />
        {!search && <div className="explorer-modes" role="group" aria-label="Result view">
          <button type="button" ref={treeButton} aria-pressed={!search && !changes} onClick={() => showSearch(false)}>Tree</button>
          {comparing && (
            <button type="button" aria-pressed={changes} onClick={showChanges}>{strings.changesMode}</button>
          )}
        </div>}
        <div className="explorer-view" hidden={search || changes}>
          <TreePanel root={root} scanId={scanId} />
        </div>
        {!search && changes && (
          <div className="explorer-view">
            <ChangesList scanId={scanId} active={changes} opening={opening} navigationError={error}
              onOpen={(id) => void openResult(id)} />
          </div>
        )}
        <div className="explorer-view" hidden={!search}>
          <FileSearch scanId={scanId} active={search} opening={opening} navigationError={error}
            onOpen={(id) => void openResult(id)} />
        </div>
      </section>
      {!search && !mapOnly && <Splitter horizontal value={treeShare} min={0.25} max={0.8} onChange={setTreeShare} />}
      {!search && !mapOnly && <aside className="inspector">
        <h2>{selected !== null ? 'Selection details' : focusId === root.id ? 'Scan details' : 'Folder details'}</h2>
        <NodeDetailsPanel key={`${selected ?? focusId ?? root.id}:${generation}`} scanId={scanId} id={selected ?? focusId ?? root.id} scanSize={root.size} />
      </aside>}
    </main>
  );
}
