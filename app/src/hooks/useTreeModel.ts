import { useCallback, useEffect, useRef, useState } from 'react';
import { getChildren, type NodeView } from '../api';
import type { Crumb } from '../store';
import { MAX_REVEAL_PAGES, PAGE_SIZE, TreeModel, type Row } from '../treeModel';

export interface TreeController {
  model: TreeModel;
  rows: Row[];
  /** Fetches the next page of `id`'s children (deduplicated). */
  loadMore(id: number): Promise<void>;
  setExpanded(id: number, expanded: boolean): void;
  /**
   * Expands every node on `path` (root first), loading pages until each next node is loaded.
   * Resolves to false when the last node could not be loaded (see `TreeModel.reveal`).
   */
  revealPath(path: number[], cancelled: () => boolean, positions?: (number | null)[]): Promise<boolean>;
  /** Root..id as breadcrumbs, or null if `id` is not loaded. */
  crumbsOf(id: number): Crumb[] | null;
}

/** Owns the lazily loaded tree of one scan result; mount it again for a new result. */
export function useTreeModel(root: NodeView, scanId: number): TreeController {
  const [model] = useState(() => {
    const m = new TreeModel(root);
    m.get(root.id)!.expanded = true;
    return m;
  });
  const [rows, setRows] = useState<Row[]>(() => model.rows());
  const inflight = useRef(new Map<number, Promise<void>>());

  const refresh = useCallback(() => setRows(model.rows()), [model]);

  const loadMore = useCallback(
    (id: number): Promise<void> => {
      const entry = model.get(id);
      if (!entry || entry.complete || entry.failed) return Promise.resolve();
      const running = inflight.current.get(id);
      if (running) return running;
      entry.loading = true;
      const offset = entry.children.length;
      const p = getChildren(id, 'SizeDesc', offset, PAGE_SIZE, scanId)
        .then(
          (page) => {
            model.addPage(id, offset, page);
          },
          () => {
            entry.failed = true;
          },
        )
        .finally(() => {
          entry.loading = false;
          inflight.current.delete(id);
          refresh();
        });
      inflight.current.set(id, p);
      return p;
    },
    [model, refresh, scanId],
  );

  const setExpanded = useCallback(
    (id: number, expanded: boolean) => {
      const entry = model.get(id);
      if (!entry || !model.canExpand(id) || entry.expanded === expanded) return;
      entry.expanded = expanded;
      refresh();
      if (expanded && entry.children.length === 0) void loadMore(id);
    },
    [model, refresh, loadMore],
  );

  const revealPath = useCallback(
    (path: number[], cancelled: () => boolean, positions?: (number | null)[]) => {
      const loadLocated = positions && (async (id: number, step: number) => {
        const position = positions[step];
        if (position === null || position === undefined) return;
        const offset = Math.floor(position / PAGE_SIZE) * PAGE_SIZE;
        try {
          const page = await getChildren(id, 'SizeDesc', offset, PAGE_SIZE, scanId);
          if (!cancelled()) {
            model.addLocatedPage(id, offset, page);
            refresh();
          }
        } catch {
          // A replacement scan or a failed page cannot reveal this node.
        }
      });
      return model.reveal(path, loadMore, refresh, MAX_REVEAL_PAGES, cancelled, loadLocated);
    },
    [model, refresh, loadMore, scanId],
  );

  const crumbsOf = useCallback(
    (id: number): Crumb[] | null => {
      const path = model.pathTo(id);
      return path && path.map((p) => ({ id: p, name: model.get(p)!.node.name }));
    },
    [model],
  );

  // First page of the root.
  useEffect(() => {
    void loadMore(model.rootId);
  }, [model, loadMore]);

  return { model, rows, loadMore, setExpanded, revealPath, crumbsOf };
}
