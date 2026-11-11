/**
 * The loaded part of the scanned tree as the folder tree shows it.
 *
 * Only pages that were fetched are kept, so memory follows what the user opened, not the
 * size of the scan. `rows()` flattens the expanded part into the list the virtual list renders.
 */
import type { NodeView } from './api';

/** Children are fetched in pages of this many nodes. */
export const PAGE_SIZE = 200;

export interface TreeEntry {
  node: NodeView;
  parent: number | null;
  /** 1 for the root (matches `aria-level`). */
  level: number;
  /** Ids of the loaded children, in display order. */
  children: number[];
  /** All children are loaded (always true for files). */
  complete: boolean;
  loading: boolean;
  expanded: boolean;
  failed: boolean;
}

/** One line of the flattened tree: a node, or the "loading more" line of a directory. */
export type Row =
  | { kind: 'node'; id: number; level: number }
  | { kind: 'more'; parent: number; level: number };

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
    for (const node of page) {
      if (!this.entries.has(node.id)) {
        this.entries.set(node.id, TreeModel.entry(node, parentId, parent.level + 1));
      }
      parent.children.push(node.id);
    }
    if (page.length < pageSize) parent.complete = true;
    return true;
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

  /** The visible lines: expanded directories show their loaded children plus a "more" line. */
  rows(): Row[] {
    const out: Row[] = [];
    // Explicit stack: folder chains can be thousands of levels deep.
    type Item = { kind: 'node'; id: number } | { kind: 'more'; parent: number; level: number };
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
      if (!e.complete) stack.push({ kind: 'more', parent: item.id, level: e.level + 1 });
      for (let i = e.children.length - 1; i >= 0; i -= 1) {
        stack.push({ kind: 'node', id: e.children[i] });
      }
    }
    return out;
  }
}
