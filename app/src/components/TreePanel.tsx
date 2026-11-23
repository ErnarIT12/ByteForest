import { useVirtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';
import type { NodeView } from '../api';
import { useTreeModel } from '../hooks/useTreeModel';
import { deleteKeyIntent, isContextMenuKey } from '../contextMenu';
import { guardWritable } from '../readOnly';
import { useDiscardPile } from '../discardPile';
import { useFileActions } from '../fileActions';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { MoreRow, TreeRow } from './TreeRow';

const ROW_HEIGHT = 28;

/** Lazily loaded, virtualized folder tree of one scan result. */
export function TreePanel({ root, scanId }: { root: NodeView; scanId: number }) {
  const tree = useTreeModel(root, scanId);
  const { model, rows, loadMore, setExpanded, revealPath, crumbsOf } = tree;
  const selectedId = useAppStore((s) => s.selectedId);
  const focusId = useAppStore((s) => s.focusId);
  const reveal = useAppStore((s) => s.reveal);
  const select = useAppStore((s) => s.select);
  const comparing = useAppStore((s) => s.baseline !== null);
  const queued = useDiscardPile((s) => s.idSet);
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
  // Search positions let the tree jump directly to a distant page.
  useEffect(() => {
    if (!reveal) return;
    let alive = true;
    revealPath(reveal.path, () => !alive, reveal.positions).then((found) => {
      if (alive) setScrollTarget(found ? { id: reveal.path[reveal.path.length - 1] } : null);
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
    .filter((r) => r?.kind === 'more' && !r.gap)
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

  /**
   * Row of the keyboard cursor: the selected node, or, when it is not in the list (not loaded
   * or collapsed away), its deepest visible ancestor on the last reveal path, then the focus.
   */
  const cursorRow = (): number => {
    if (selectedId === null) return -1;
    const direct = rowIndex.get(selectedId);
    if (direct !== undefined) return direct;
    const path = reveal?.path ?? [];
    for (let k = path.length - 1; k >= 0; k -= 1) {
      const i = rowIndex.get(path[k]);
      if (i !== undefined) return i;
    }
    return focusId === null ? -1 : (rowIndex.get(focusId) ?? -1);
  };

  const openMenu = useCallback(
    (id: number, x: number, y: number) => void useFileActions.getState().openMenu(scanId, id, x, y),
    [scanId],
  );

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (isContextMenuKey(e)) {
      if (selectedId === null || !rowIndex.has(selectedId)) return;
      e.preventDefault();
      const box = (
        document.getElementById(`tree-row-${selectedId}`) ?? e.currentTarget
      ).getBoundingClientRect();
      openMenu(selectedId, box.left + 32, box.bottom);
      return;
    }
    const intent = deleteKeyIntent(e);
    if (intent) {
      // Delete only queues; Shift+Delete asks before moving to the Recycle Bin / Trash.
      if (selectedId === null || !rowIndex.has(selectedId) || selectedId === model.rootId) return;
      e.preventDefault();
      if (!guardWritable()) return;
      if (intent === 'queue') useDiscardPile.getState().add([selectedId]);
      else void useFileActions.getState().requestTrash(scanId, selectedId);
      return;
    }
    const cur = cursorRow();
    const row = cur >= 0 ? rows[cur] : undefined;
    const curId = row?.kind === 'node' ? row.id : null;
    const entry = curId === null ? undefined : model.get(curId);
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
        if (!entry || curId === null) return;
        if (curId !== selectedId) moveTo(cur);
        else if (model.canExpand(curId) && !entry.expanded) setExpanded(curId, true);
        else if (entry.expanded && entry.children.length > 0) moveTo(cur + 1);
        break;
      case 'ArrowLeft':
        if (!entry || curId === null) return;
        if (curId !== selectedId) moveTo(cur);
        else if (entry.expanded) setExpanded(curId, false);
        else if (entry.parent !== null) moveTo(rowIndex.get(entry.parent) ?? cur);
        break;
      case 'Enter':
        if (curId !== null && curId === selectedId) activate(curId);
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  return (
    <div className={comparing ? 'tree-panel comparing' : 'tree-panel'}>
      <div className="tree-header" aria-hidden="true">
        <div className="cell name">{strings.colName}</div>
        <div className="cell num">{strings.colSize}</div>
        {comparing && <div className="cell num col-delta">{strings.colChange}</div>}
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
                <MoreRow key={`more-${r.parent}`} level={r.level} failed={!!model.get(r.parent)?.failed} top={it.start}
                  onLoad={r.gap ? () => void loadMore(r.parent) : undefined} />
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
                showDelta={comparing}
                selected={r.id === selectedId}
                focused={r.id === focusId}
                queued={queued.has(r.id)}
                top={it.start}
                onSelect={select}
                onToggle={toggle}
                onActivate={activate}
                onContextMenu={openMenu}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
}
