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
      // Flexible tracks share the width split : (1 - split) whatever the content; the
      // minimums keep both panes usable in a narrow window.
      style={{ gridTemplateColumns: `minmax(300px, ${split}fr) 5px minmax(320px, ${1 - split}fr)` }}
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
