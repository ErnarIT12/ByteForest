import { useRef, useState } from 'react';
import { locateNode, type NodeView } from '../api';
import { useAppStore } from '../store';
import { Breadcrumbs } from './Breadcrumbs';
import { FileSearch } from './FileSearch';
import { NodeDetailsPanel } from './NodeDetailsPanel';
import { Splitter } from './Splitter';
import { TreemapCanvas } from './TreemapCanvas';
import { TreePanel } from './TreePanel';

/** The Explorer screen: folder tree | breadcrumbs over the treemap. */
export function Explorer({ root, scanId, split, onSplitChange }: {
  root: NodeView;
  scanId: number;
  split: number;
  onSplitChange: (value: number) => void;
}) {
  const [search, setSearch] = useState(false);
  const [opening, setOpening] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const request = useRef(0);
  const treeButton = useRef<HTMLButtonElement>(null);
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
      className="explorer"
      // Flexible tracks share the width split : (1 - split) whatever the content; the
      // minimums keep both panes usable in a narrow window.
      style={{ gridTemplateColumns: `minmax(300px, ${split}fr) 5px minmax(320px, ${1 - split}fr)` }}
    >
      <section className="explorer-sidebar" aria-label="Scan results">
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
      <Splitter value={split} min={0.2} max={0.7} onChange={onSplitChange} />
      <section className="map-panel">
        <Breadcrumbs />
        <TreemapCanvas />
        {selected !== null && <NodeDetailsPanel key={selected} scanId={scanId} id={selected} />}
      </section>
    </main>
  );
}
