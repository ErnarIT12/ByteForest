import { useVirtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import type { NodeView } from '../api';
import { useTreeModel } from '../hooks/useTreeModel';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { MoreRow, TreeRow } from './TreeRow';

const ROW_HEIGHT = 28;

/** Lazily loaded, virtualized folder tree of one scan result. */
export function TreePanel({ root }: { root: NodeView }) {
  const tree = useTreeModel(root);
  const { model, rows, loadMore, setExpanded, revealPath, crumbsOf } = tree;
  const selectedId = useAppStore((s) => s.selectedId);
  const focusId = useAppStore((s) => s.focusId);
  const reveal = useAppStore((s) => s.reveal);
  const select = useAppStore((s) => s.select);
  const setFocus = useAppStore((s) => s.setFocus);

  const scrollRef = useRef<HTMLDivElement>(null);
  /** Node to scroll to once the rows that contain it are rendered. */
  const [scrollTarget, setScrollTarget] = useState<{ id: number } | null>(null);
  const scrolledTo = useRef<{ id: number } | null>(null);

  // The app does not use React Compiler; the virtualizer re-renders this component itself.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });

  const rowIndex = useMemo(() => {
    const m = new Map<number, number>();
    rows.forEach((r, i) => r.kind === 'node' && m.set(r.id, i));
    return m;
  }, [rows]);

  // Reveal requests (zoom from the treemap, a clicked file): expand the path, then scroll.
  useEffect(() => {
    if (!reveal) return;
    let alive = true;
    revealPath(reveal.path).then(() => {
      if (alive) setScrollTarget({ id: reveal.path[reveal.path.length - 1] });
    });
    return () => {
      alive = false;
    };
  }, [reveal, revealPath]);

  useEffect(() => {
    if (!scrollTarget || scrolledTo.current === scrollTarget) return;
    const i = rowIndex.get(scrollTarget.id);
    if (i !== undefined) {
      scrolledTo.current = scrollTarget;
      virtualizer.scrollToIndex(i, { align: 'auto' });
    }
  }, [scrollTarget, rowIndex, virtualizer]);

  // Load the next page when a directory's "loading" line scrolls into view.
  const items = virtualizer.getVirtualItems();
  const visibleMore = items
    .map((it) => rows[it.index])
    .filter((r) => r?.kind === 'more')
    .map((r) => (r.kind === 'more' ? r.parent : -1))
    .join(',');
  useEffect(() => {
    if (!visibleMore) return;
    for (const id of visibleMore.split(',')) void loadMore(Number(id));
  }, [visibleMore, loadMore]);

  const toggle = useCallback(
    (id: number) => setExpanded(id, !model.get(id)?.expanded),
    [model, setExpanded],
  );
  const activate = useCallback(
    (id: number) => {
      const path = crumbsOf(id);
      if (path && model.get(id)?.node.is_dir) setFocus(id, path);
    },
    [crumbsOf, model, setFocus],
  );

  const moveTo = (i: number) => {
    const r = rows[Math.max(0, Math.min(rows.length - 1, i))];
    if (!r || r.kind !== 'node') return;
    select(r.id);
    virtualizer.scrollToIndex(rowIndex.get(r.id) ?? 0, { align: 'auto' });
  };

  /** Next node row from `i` in direction `step` (skips "loading" lines). */
  const nodeRowFrom = (i: number, step: number): number => {
    let j = i + step;
    while (j >= 0 && j < rows.length && rows[j].kind !== 'node') j += step;
    return j;
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const cur = selectedId === null ? -1 : (rowIndex.get(selectedId) ?? -1);
    const entry = selectedId === null ? undefined : model.get(selectedId);
    switch (e.key) {
      case 'ArrowDown':
        moveTo(cur < 0 ? 0 : nodeRowFrom(cur, 1));
        break;
      case 'ArrowUp':
        moveTo(cur < 0 ? 0 : nodeRowFrom(cur, -1));
        break;
      case 'Home':
        moveTo(0);
        break;
      case 'End':
        moveTo(nodeRowFrom(rows.length, -1));
        break;
      case 'ArrowRight':
        if (!entry || selectedId === null) return;
        if (model.canExpand(selectedId) && !entry.expanded) setExpanded(selectedId, true);
        else if (entry.expanded && entry.children.length > 0) moveTo(cur + 1);
        break;
      case 'ArrowLeft':
        if (!entry || selectedId === null) return;
        if (entry.expanded) setExpanded(selectedId, false);
        else if (entry.parent !== null) moveTo(rowIndex.get(entry.parent) ?? cur);
        break;
      case 'Enter':
        if (selectedId !== null) activate(selectedId);
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  return (
    <div className="tree-panel">
      <div className="tree-header" aria-hidden="true">
        <div className="cell name">{strings.colName}</div>
        <div className="cell num">{strings.colSize}</div>
        <div className="cell percent">{strings.colPercent}</div>
        <div className="cell num col-files">{strings.colFiles}</div>
        <div className="cell date col-modified">{strings.colModified}</div>
      </div>
      <div
        ref={scrollRef}
        className="tree-scroll"
        role="tree"
        aria-label={strings.treeLabel}
        tabIndex={0}
        aria-activedescendant={selectedId !== null && rowIndex.has(selectedId) ? `tree-row-${selectedId}` : undefined}
        onKeyDown={onKeyDown}
      >
        <div className="tree-canvas" style={{ height: virtualizer.getTotalSize() }}>
          {items.map((it) => {
            const r = rows[it.index];
            if (!r) return null;
            if (r.kind === 'more') {
              return (
                <MoreRow key={`more-${r.parent}`} level={r.level} failed={!!model.get(r.parent)?.failed} top={it.start} />
              );
            }
            const entry = model.get(r.id)!;
            return (
              <TreeRow
                key={r.id}
                node={entry.node}
                expanded={entry.expanded}
                isRoot={entry.parent === null}
                level={r.level}
                selected={r.id === selectedId}
                focused={r.id === focusId}
                top={it.start}
                onSelect={select}
                onToggle={toggle}
                onActivate={activate}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}
