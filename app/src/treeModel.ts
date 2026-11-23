/**
 * The loaded part of the scanned tree as the folder tree shows it.
 *
 * Only pages that were fetched are kept, so memory follows what the user opened, not the
 * size of the scan. `rows()` flattens the expanded part into the list the virtual list renders.
 */
import type { NodeView } from './api';

/** Children are fetched in pages of this many nodes. */
export const PAGE_SIZE = 200;

/** Pages `reveal` loads under one parent before giving up (10 000 rows). */
export const MAX_REVEAL_PAGES = 50;

export interface TreeEntry {
  node: NodeView;
  parent: number | null;
  /** 1 for the root (matches `aria-level`). */
  level: number;
  /** Ids of the loaded children, in display order. */
  children: number[];
  /** Pages loaded ahead of the contiguous prefix to reveal a search result. */
  locatedPages?: Map<number, NodeView[]>;
  /** All children are loaded (always true for files). */
  complete: boolean;
  loading: boolean;
  expanded: boolean;
  failed: boolean;
}

/** One line of the flattened tree: a node, or the "loading more" line of a directory. */
export type Row =
  | { kind: 'node'; id: number; level: number }
  | { kind: 'more'; parent: number; level: number; gap?: boolean };

export class TreeModel {
  readonly entries = new Map<number, TreeEntry>();
  readonly rootId: number;

  constructor(root: NodeView) {
    this.rootId = root.id;
    this.entries.set(root.id, TreeModel.entry(root, null, 1));
  }

  private static entry(node: NodeView, parent: number | null, level: number): TreeEntry {
    const leaf = !node.is_dir || !node.has_children;
    return {
      node,
      parent,
      level,
      children: [],
      complete: leaf,
      loading: false,
      expanded: false,
      failed: false,
    };
  }

  get(id: number): TreeEntry | undefined {
    return this.entries.get(id);
  }

  /** Whether `id` is a directory that can be opened. */
  canExpand(id: number): boolean {
    const e = this.entries.get(id);
    return !!e && e.node.is_dir && e.node.has_children;
  }

  /**
   * Stores a page fetched at `offset`. A page that does not continue the loaded list
   * (a duplicate or stale response) is ignored. Returns whether it was applied.
   */
  addPage(parentId: number, offset: number, page: NodeView[], pageSize = PAGE_SIZE): boolean {
    const parent = this.entries.get(parentId);
    if (!parent || parent.complete || offset !== parent.children.length) return false;
    this.registerChildren(parentId, parent, page);
    parent.children.push(...page.map((node) => node.id));
    if (page.length < pageSize) parent.complete = true;
    while (!parent.complete && parent.locatedPages?.has(parent.children.length)) {
      const offset = parent.children.length;
      const located = parent.locatedPages.get(offset)!;
      parent.locatedPages.delete(offset);
      parent.children.push(...located.map((node) => node.id));
      if (located.length < pageSize) parent.complete = true;
    }
    return true;
  }

  addLocatedPage(parentId: number, offset: number, page: NodeView[]): void {
    const parent = this.entries.get(parentId);
    if (!parent || parent.complete || offset < parent.children.length) return;
    if (offset === parent.children.length) {
      this.addPage(parentId, offset, page);
      return;
    }
    if (!page.length) return;
    this.registerChildren(parentId, parent, page);
    (parent.locatedPages ??= new Map()).set(offset, page);
  }

  private registerChildren(parentId: number, parent: TreeEntry, page: NodeView[]): void {
    for (const node of page) {
      if (!this.entries.has(node.id)) {
        this.entries.set(node.id, TreeModel.entry(node, parentId, parent.level + 1));
      }
    }
  }

  /** Replaces the stored view of a loaded node (sizes changed); keeps its tree state. */
  setNode(node: NodeView): void {
    const e = this.entries.get(node.id);
    if (e) e.node = node;
  }

  /**
   * Replaces the loaded children of `parentId` with a fresh list (same sort order), keeping
   * the entries, and so the expansion state, of the children that are still there. Entries of
   * children that disappeared are dropped together with their loaded descendants.
   */
  replaceChildren(parentId: number, nodes: NodeView[], complete: boolean): void {
    const parent = this.entries.get(parentId);
    if (!parent) return;
    const keep = new Set(nodes.map((n) => n.id));
    const stale = [...parent.children, ...[...(parent.locatedPages?.values() ?? [])].flatMap((p) => p.map((n) => n.id))];
    parent.locatedPages = undefined;
    for (const id of stale) if (!keep.has(id)) this.dropSubtree(id);
    for (const node of nodes) {
      const existing = this.entries.get(node.id);
      if (existing) existing.node = node;
      else this.entries.set(node.id, TreeModel.entry(node, parentId, parent.level + 1));
    }
    parent.children = nodes.map((n) => n.id);
    parent.complete = complete || !parent.node.is_dir || !parent.node.has_children;
    parent.failed = false;
  }

  /** Forgets everything but the root, which gets `root` as its fresh view and no children. */
  resetPages(root: NodeView): void {
    const entry = this.entries.get(this.rootId);
    if (!entry) return;
    this.entries.clear();
    this.entries.set(this.rootId, { ...TreeModel.entry(root, null, 1), expanded: entry.expanded });
  }

  /** Removes the entry and every loaded descendant (iterative: chains can be very deep). */
  private dropSubtree(id: number): void {
    const stack = [id];
    while (stack.length > 0) {
      const e = this.entries.get(stack.pop()!);
      if (!e) continue;
      stack.push(...e.children);
      for (const page of e.locatedPages?.values() ?? []) stack.push(...page.map((n) => n.id));
      this.entries.delete(e.node.id);
    }
  }

  /** Ids from the root down to `id`, or null if `id` is not loaded. */
  pathTo(id: number): number[] | null {
    const path: number[] = [];
    let cur: number | null = id;
    while (cur !== null) {
      const e = this.entries.get(cur);
      if (!e) return null;
      path.push(cur);
      cur = e.parent;
    }
    return path.reverse();
  }

  /**
   * Expands every node on `path` (root first), calling `loadPage(parent)` until the next node
   * of the path is loaded. Gives up (returns false) when a node cannot be found: the parent is
   * complete or failed, more than `maxPages` pages would be needed, or `cancelled()` turns true.
   * Returns true when the last node of `path` is loaded and every ancestor is expanded.
   */
  async reveal(
    path: number[],
    loadPage: (parent: number) => Promise<void>,
    onChange: () => void,
    maxPages = MAX_REVEAL_PAGES,
    cancelled: () => boolean = () => false,
    loadLocated?: (parent: number, step: number) => Promise<void>,
  ): Promise<boolean> {
    if (path.length === 0 || path[0] !== this.rootId) return false;
    for (let i = 0; i < path.length; i += 1) {
      const id = path[i];
      const entry = this.entries.get(id);
      if (!entry) return false;
      if (this.canExpand(id) && !entry.expanded) {
        entry.expanded = true;
        onChange();
      }
      if (i === path.length - 1) {
        if (this.canExpand(id) && entry.children.length === 0) await loadPage(id);
        return !cancelled();
      }
      const next = path[i + 1];
      if (loadLocated && !this.entries.has(next) && !cancelled()) {
        await loadLocated(id, i + 1);
        if (!this.entries.has(next)) return false;
      }
      let pages = 0;
      while (!this.entries.has(next)) {
        if (entry.complete || entry.failed || pages >= maxPages || cancelled()) return false;
        await loadPage(id);
        pages += 1;
      }
      if (cancelled()) return false;
    }
    return true;
  }

  /** The visible lines: expanded directories show their loaded children plus a "more" line. */
  rows(): Row[] {
    const out: Row[] = [];
    // Explicit stack: folder chains can be thousands of levels deep.
    type Item = { kind: 'node'; id: number } | Extract<Row, { kind: 'more' }>;
    const stack: Item[] = [{ kind: 'node', id: this.rootId }];
    while (stack.length > 0) {
      const item = stack.pop()!;
      if (item.kind === 'more') {
        out.push(item);
        continue;
      }
      const e = this.entries.get(item.id);
      if (!e) continue;
      out.push({ kind: 'node', id: item.id, level: e.level });
      if (!e.expanded) continue;
      const more = { kind: 'more' as const, parent: item.id, level: e.level + 1 };
      const located = [...(e.locatedPages ?? [])].sort(([a], [b]) => a - b);
      if (!e.complete && !located.length) stack.push(more);
      for (let p = located.length - 1; p >= 0; p -= 1) {
        const [offset, page] = located[p];
        for (let j = page.length - 1; j >= 0; j -= 1) {
          stack.push({ kind: 'node', id: page[j].id });
        }
        if (p === 0 && offset > e.children.length) stack.push({ ...more, gap: true });
      }
      for (let i = e.children.length - 1; i >= 0; i -= 1) {
        stack.push({ kind: 'node', id: e.children[i] });
      }
    }
    return out;
  }
}
