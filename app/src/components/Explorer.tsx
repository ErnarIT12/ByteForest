import { useState } from 'react';
import type { NodeView } from '../api';
import { Breadcrumbs } from './Breadcrumbs';
import { Splitter } from './Splitter';
import { TreemapCanvas } from './TreemapCanvas';
import { TreePanel } from './TreePanel';

const DEFAULT_SPLIT = 0.4;

/** The Explorer screen: folder tree | breadcrumbs over the treemap. */
export function Explorer({ root, generation }: { root: NodeView; generation: number }) {
  const [split, setSplit] = useState(DEFAULT_SPLIT);
  return (
    <main
      className="explorer"
      style={{ gridTemplateColumns: `minmax(300px, ${split * 100}%) auto minmax(320px, 1fr)` }}
    >
      <TreePanel key={generation} root={root} />
      <Splitter value={split} min={0.2} max={0.7} onChange={setSplit} />
      <section className="map-panel">
        <Breadcrumbs />
        <TreemapCanvas />
      </section>
    </main>
  );
}
