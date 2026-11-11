import { useCallback, useEffect, useRef, useState } from 'react';
import { getChildren, type NodeView } from '../api';
import type { Crumb } from '../store';
import { PAGE_SIZE, TreeModel, type Row } from '../treeModel';

export interface TreeController {
  model: TreeModel;
  rows: Row[];
  /** Fetches the next page of `id`'s children (deduplicated). */
  loadMore(id: number): Promise<void>;
  setExpanded(id: number, expanded: boolean): void;
  /** Expands every node on `path` (root first), loading first pages as needed. */
  revealPath(path: number[]): Promise<void>;
  /** Root..id as breadcrumbs, or null if `id` is not loaded. */
  crumbsOf(id: number): Crumb[] | null;
}

/** Owns the lazily loaded tree of one scan result; mount it again for a new result. */
export function useTreeModel(root: NodeView): TreeController {
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
      const p = getChildren(id, 'SizeDesc', offset, PAGE_SIZE)
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
    [model, refresh],
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
    async (path: number[]) => {
      for (let i = 0; i < path.length; i += 1) {
        const id = path[i];
        const entry = model.get(id);
        // Not loaded: the parent's first page did not include it. Stop here.
        if (!entry) return;
        if (model.canExpand(id) && !entry.expanded) {
          entry.expanded = true;
          refresh();
        }
        if (model.canExpand(id) && entry.children.length === 0) await loadMore(id);
      }
    },
    [model, refresh, loadMore],
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
