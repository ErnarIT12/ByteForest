import { useCallback, useEffect, useRef, useState } from 'react';
import { getChildren, type NodeView } from '../api';
import { useAppStore, type Crumb, type TreeInvalidation } from '../store';
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

  // A removal changed sizes along the path to the root: fetch the loaded pages of those
  // directories again, keeping the expanded state of every other node.
  const invalidation = useAppStore((s) => s.invalidation);
  const handled = useRef(invalidation?.seq ?? 0);
  useEffect(() => {
    if (!invalidation || invalidation.seq === handled.current) return;
    handled.current = invalidation.seq;
    let alive = true;
    void refreshAfterRemoval(model, invalidation, scanId, () => alive, inflight.current).then((ok) => {
      if (!alive) return;
      refresh();
      if (!ok) void loadMore(model.rootId);
    });
    return () => {
      alive = false;
    };
  }, [invalidation, model, scanId, refresh, loadMore]);

  // Starting or stopping a snapshot comparison only changes the `diff` of the rows: refetch
  // the loaded pages in place so expansion and selection survive.
  const baselineSeq = useAppStore((s) => s.baselineSeq);
  const seenBaseline = useRef(baselineSeq);
  useEffect(() => {
    if (baselineSeq === seenBaseline.current) return;
    seenBaseline.current = baselineSeq;
    let alive = true;
    void Promise.all([...inflight.current.values()]).then(async () => {
      if (!alive) return;
      await model.refreshViews((id, offset) => getChildren(id, 'SizeDesc', offset, PAGE_SIZE, scanId), () => alive);
      if (alive) refresh();
    });
    return () => {
      alive = false;
    };
  }, [baselineSeq, model, scanId, refresh]);

  // First page of the root.
  useEffect(() => {
    void loadMore(model.rootId);
  }, [model, loadMore]);

  return { model, rows, loadMore, setExpanded, revealPath, crumbsOf };
}

/**
 * Re-reads the loaded children of the removed node's parent and of all its ancestors, from the
 * root down. Returns false when the model had to be reset instead (the parent was not loaded
 * or a page failed); the caller then loads the root again.
 */
async function refreshAfterRemoval(
  model: TreeModel,
  inv: TreeInvalidation,
  scanId: number,
  alive: () => boolean,
  inflight: Map<number, Promise<void>>,
): Promise<boolean> {
  const root = useAppStore.getState().root;
  if (!root) return true;
  const chain = model.pathTo(inv.parentId);
  const reset = () => {
    model.resetPages(root);
    return false;
  };
  if (!chain) return reset();
  model.setNode(root);
  try {
    const fresh = await Promise.all(
      chain.map(async (id) => {
        await inflight.get(id);
        const loaded = model.get(id)?.children.length ?? 0;
        if (loaded === 0) return null;
        const nodes: NodeView[] = [];
        let complete = false;
        while (nodes.length < loaded) {
          const page = await getChildren(id, 'SizeDesc', nodes.length, PAGE_SIZE, scanId);
          nodes.push(...page);
          if (page.length < PAGE_SIZE) {
            complete = true;
            break;
          }
        }
        return { id, nodes, complete };
      }),
    );
    if (!alive()) return true;
    for (const item of fresh) if (item && model.get(item.id)) model.replaceChildren(item.id, item.nodes, item.complete);
    return true;
  } catch {
    return alive() ? reset() : true;
  }
}
