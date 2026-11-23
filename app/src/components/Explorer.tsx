import { useRef, useState } from 'react';
import { locateNode, type NodeView } from '../api';
import { formatBytes, formatInt } from '../format';
import { useAppStore } from '../store';
import { Breadcrumbs } from './Breadcrumbs';
import { FileSearch } from './FileSearch';
import { NodeDetailsPanel } from './NodeDetailsPanel';
import { Splitter } from './Splitter';
import { TreemapCanvas } from './TreemapCanvas';
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
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const request = useRef(0);
  const treeButton = useRef<HTMLButtonElement>(null);
  const focusId = useAppStore((s) => s.focusId);
  const selected = useAppStore((s) => s.selectedId);

  const showSearch = (value: boolean) => {
    request.current += 1;
    setOpening(false);
    setError(null);
    setSearch(value);
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
      style={{ gridTemplateColumns: mapOnly ? 'minmax(0, 1fr)' : `minmax(440px, ${split}fr) 8px minmax(235px, ${1 - split}fr)` }}
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
          <button type="button" className="map-mode" aria-controls="result-browser"
            aria-expanded={!mapOnly} onClick={() => setMapOnly(!mapOnly)}>
            {mapOnly ? 'Show file list' : 'Expand map'}
          </button>
        </div>
        <TreemapCanvas />

      </section>
      {!mapOnly && <Splitter value={split} min={0.45} max={0.82} onChange={onSplitChange} />}
      <section id="result-browser" hidden={mapOnly} className="explorer-sidebar" aria-label="Scan results">
        <div className="explorer-modes" role="group" aria-label="Result view">
          <button type="button" ref={treeButton} aria-pressed={!search} onClick={() => showSearch(false)}>Tree</button>
          <button type="button" aria-pressed={search} onClick={() => showSearch(true)}>Largest Files</button>
        </div>
        <div className="explorer-view" hidden={search}><TreePanel root={root} scanId={scanId} /></div>
        <div className="explorer-view" hidden={!search}>
          <FileSearch scanId={scanId} active={search} opening={opening} navigationError={error}
            onOpen={(id) => void openResult(id)} />
        </div>
      </section>
      {!mapOnly && <aside className="inspector">
        <h2>{selected !== null ? 'Selection details' : focusId === root.id ? 'Scan details' : 'Folder details'}</h2>
        <NodeDetailsPanel key={selected ?? focusId ?? root.id} scanId={scanId} id={selected ?? focusId ?? root.id} scanSize={root.size} />
      </aside>}
    </main>
  );
}
