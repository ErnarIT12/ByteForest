import { useEffect, useRef, useState } from 'react';
import { setTreemapColorMode, useTreemapColorMode } from '../hooks/useTreemapColorMode';
import { locateNode, type NodeView } from '../api';
import { formatBytes, formatInt } from '../format';
import { useAppStore } from '../store';
import { Breadcrumbs } from './Breadcrumbs';
import { ChangesList } from './ChangesList';
import { CompareBanner } from './CompareBanner';
import { FileSearch } from './FileSearch';
import { NodeDetailsPanel } from './NodeDetailsPanel';
import { strings } from '../strings';
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
  const [mapOnly, setMapOnly] = useState(false);
  const [search, setSearch] = useState(false);
  const [changesMode, setChangesMode] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
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
      setSearch(true);
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
      setSearch(false);
      setChangesMode(false);
      setMapOnly(false);
    }
  }

  // Drops a pending "locate file" request so it cannot switch the pane back.
  useEffect(() => {
    request.current += 1;
  }, [presetSeq]);

  const showSearch = (value: boolean) => {
    request.current += 1;
    setOpening(false);
    setError(null);
    setSearch(value);
    setChangesMode(false);
  };

  const showChanges = () => {
    request.current += 1;
    setOpening(false);
    setError(null);
    setSearch(false);
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
      setSearch(false);
      setChangesMode(false);
      treeButton.current?.focus();
    } catch (failure) {
      if (isCurrent()) setError(String(failure));
    } finally {
      if (isCurrent()) setOpening(false);
    }
  };
  return (
    <main
      className={mapOnly ? "explorer map-expanded" : "explorer"}
      // Flexible tracks share the width split : (1 - split) whatever the content; the
      // minimums keep both panes usable in a narrow window.
      style={{ gridTemplateColumns: mapOnly ? 'minmax(0, 1fr)' : `minmax(400px, ${split}fr) 8px minmax(235px, ${1 - split}fr)` }}
    >
      <header className="scan-heading">
        <h1 title={root.name}>{root.name}</h1>
        <dl className="scan-totals">
          <div><dt>Scanned</dt><dd>{formatBytes(root.size)}</dd></div>
          <div><dt>Files</dt><dd>{formatInt(root.files)}</dd></div>
          <div><dt>Folders</dt><dd>{formatInt(root.dirs)}</dd></div>
        </dl>
      </header>
      <section className="map-panel">
        <div className="map-toolbar">
          <Breadcrumbs />
          <span className="map-hint">Double-click a folder to open</span>
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
      </section>
      {!mapOnly && <Splitter value={split} min={0.45} max={0.82} onChange={onSplitChange} />}
      <section id="result-browser" hidden={mapOnly} className="explorer-sidebar" aria-label="Scan results">
        <CompareBanner />
        <div className="explorer-modes" role="group" aria-label="Result view">
          <button type="button" ref={treeButton} aria-pressed={!search && !changes} onClick={() => showSearch(false)}>Tree</button>
          <button type="button" aria-pressed={search} onClick={() => showSearch(true)}>Largest Files</button>
          {comparing && (
            <button type="button" aria-pressed={changes} onClick={showChanges}>{strings.changesMode}</button>
          )}
        </div>
        <div className="explorer-view" hidden={search || changes}>
          <TreePanel root={root} scanId={scanId} />
        </div>
        {changes && (
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
      {!mapOnly && <aside className="inspector">
        <h2>{selected !== null ? 'Selection details' : focusId === root.id ? 'Scan details' : 'Folder details'}</h2>
        <NodeDetailsPanel key={`${selected ?? focusId ?? root.id}:${generation}`} scanId={scanId} id={selected ?? focusId ?? root.id} scanSize={root.size} />
      </aside>}
    </main>
  );
}
