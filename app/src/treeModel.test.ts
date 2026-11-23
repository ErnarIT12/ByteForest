import { describe, expect, it } from 'vitest';
import type { NodeView } from './api';
import { PAGE_SIZE, TreeModel } from './treeModel';

const node = (id: number, isDir = true): NodeView => ({
  id,
  name: `n${id}`,
  size: 1,
  alloc: 1,
  files: 1,
  dirs: 0,
  is_dir: isDir,
  has_children: isDir,
  mtime: 0,
  percent_of_parent: 0,
});

describe('TreeModel', () => {
  it('shows a loading line until the last page arrives', () => {
    const m = new TreeModel(node(0));
    m.get(0)!.expanded = true;
    expect(m.rows()).toEqual([
      { kind: 'node', id: 0, level: 1 },
      { kind: 'more', parent: 0, level: 2 },
    ]);
    expect(m.addPage(0, 0, [node(1), node(2, false)], 2)).toBe(true);
    expect(m.rows().map((r) => r.kind)).toEqual(['node', 'node', 'node', 'more']);
    expect(m.addPage(0, 2, [node(3, false)], 2)).toBe(true);
    expect(m.rows().map((r) => (r.kind === 'node' ? r.id : -1))).toEqual([0, 1, 2, 3]);
  });

  it('ignores stale or duplicate pages', () => {
    const m = new TreeModel(node(0));
    expect(m.addPage(0, 0, [node(1)], 1)).toBe(true);
    expect(m.addPage(0, 0, [node(1)], 1)).toBe(false);
    expect(m.get(0)!.children).toEqual([1]);
  });

  it('reveal loads pages until the target on page 3 is found', async () => {
    // Root 0 > folder 1 with 650 children (ids 1000..1649); the target 1450 is on page 3.
    const m = new TreeModel(node(0));
    const kids = Array.from({ length: 650 }, (_, i) => node(1000 + i, false));
    const loads: number[] = [];
    const loadPage = async (parent: number) => {
      loads.push(parent);
      const offset = m.get(parent)!.children.length;
      const all = parent === 0 ? [node(1)] : kids;
      m.addPage(parent, offset, all.slice(offset, offset + PAGE_SIZE));
    };
    expect(await m.reveal([0, 1, 1450], loadPage, () => {})).toBe(true);
    expect(loads).toEqual([0, 1, 1, 1]);
    expect(m.get(1)!.expanded).toBe(true);
    expect(m.pathTo(1450)).toEqual([0, 1, 1450]);
    expect(m.rows().some((r) => r.kind === 'node' && r.id === 1450)).toBe(true);
  });

  it('reveal gives up past the page cap, on a complete parent or when cancelled', async () => {
    const make = () => {
      const m = new TreeModel(node(0));
      const kids = Array.from({ length: 650 }, (_, i) => node(1000 + i, false));
      const loadPage = async (parent: number) => {
        const offset = m.get(parent)!.children.length;
        m.addPage(parent, offset, kids.slice(offset, offset + PAGE_SIZE));
      };
      return { m, loadPage };
    };
    const capped = make();
    expect(await capped.m.reveal([0, 1450], capped.loadPage, () => {}, 2)).toBe(false);
    expect(capped.m.get(0)!.children.length).toBe(400);

    const missing = make();
    expect(await missing.m.reveal([0, 9999], missing.loadPage, () => {})).toBe(false);
    expect(missing.m.get(0)!.complete).toBe(true);

    const cancelled = make();
    expect(await cancelled.m.reveal([0, 1450], cancelled.loadPage, () => {}, 50, () => true)).toBe(
      false,
    );
    expect(cancelled.m.get(0)!.children.length).toBe(0);
  });

  it('reveals a late search match without fetching the preceding pages', async () => {
    const m = new TreeModel(node(0));
    const far = 65 * PAGE_SIZE;
    const loads: number[] = [];
    const sequential = async () => { throw new Error('must not scan preceding pages'); };
    const locate = async (parent: number) => {
      loads.push(parent);
      m.addLocatedPage(parent, far, [node(42, false)]);
    };
    expect(await m.reveal([0, 42], sequential, () => {}, 1, () => false, locate)).toBe(true);
    expect(loads).toEqual([0]);
    expect(m.get(0)!.children).toEqual([]);
    expect(m.rows()).toContainEqual({ kind: 'node', id: 42, level: 2 });
    expect(m.rows()).toContainEqual({ kind: 'more', parent: 0, level: 2, gap: true });
  });

  it('merges a previously located page as the preceding gap closes', () => {
    const m = new TreeModel(node(0));
    m.get(0)!.expanded = true;
    m.addLocatedPage(0, PAGE_SIZE, [node(201, false)]);
    const contiguous = Array.from({ length: PAGE_SIZE }, (_, i) => node(i + 1, false));
    expect(m.addPage(0, 0, contiguous)).toBe(true);
    expect(m.get(0)!.complete).toBe(true);
    expect(m.rows().filter((row) => row.kind === 'node')).toHaveLength(202);
    expect(m.rows().some((row) => row.kind === 'more')).toBe(false);
  });

  it('nests expanded children in order and knows their paths', () => {
    const m = new TreeModel(node(0));
    m.get(0)!.expanded = true;
    m.addPage(0, 0, [node(1), node(2)], 10);
    m.get(1)!.expanded = true;
    m.addPage(1, 0, [node(5, false)], 10);
    expect(m.rows().map((r) => (r.kind === 'node' ? `${r.id}@${r.level}` : 'more'))).toEqual([
      '0@1',
      '1@2',
      '5@3',
      '2@2',
    ]);
    expect(m.pathTo(5)).toEqual([0, 1, 5]);
    expect(m.pathTo(99)).toBeNull();
  });
});

describe('TreeModel.replaceChildren', () => {
  it('keeps expansion of surviving children and drops removed subtrees', () => {
    const m = new TreeModel(node(0));
    m.addPage(0, 0, [node(1), node(2), node(3)], 10);
    m.addPage(1, 0, [node(4, false)], 10);
    m.get(2)!.expanded = true;
    m.replaceChildren(0, [node(2), node(3)], true);
    expect(m.get(0)!.children).toEqual([2, 3]);
    expect(m.get(2)!.expanded).toBe(true);
    expect(m.get(1)).toBeUndefined();
    expect(m.get(4)).toBeUndefined();
  });

  it('updates the stored views of children that stay', () => {
    const m = new TreeModel(node(0));
    m.addPage(0, 0, [node(1)], 10);
    m.replaceChildren(0, [{ ...node(1), size: 7 }], true);
    expect(m.get(1)!.node.size).toBe(7);
  });
});

describe('TreeModel.refreshViews', () => {
  const withDiff = (n: NodeView, delta: number): NodeView => ({
    ...n,
    diff: { status: 'grown', base: 1, delta, removed: null },
  });

  it('lists loaded page offsets, contiguous and located', () => {
    const m = new TreeModel(node(0));
    m.addPage(0, 0, [node(1), node(2)], 2);
    m.addPage(0, 2, [node(3), node(4)], 2);
    expect(m.loadedPageOffsets(0, 2)).toEqual([0, 2]);
    m.addLocatedPage(0, 6, [node(9)]);
    expect(m.loadedPageOffsets(0, 2)).toEqual([0, 2, 6]);
    expect(m.loadedPageOffsets(99)).toEqual([]);
  });

  it('adds the new fields to loaded rows and keeps expansion and structure', async () => {
    const m = new TreeModel(node(0));
    m.get(0)!.expanded = true;
    m.addPage(0, 0, [node(1), node(2)], 10);
    m.addPage(1, 0, [node(3, false)], 10);
    m.get(1)!.expanded = true;
    const before = m.rows();
    const asked: [number, number][] = [];
    const ok = await m.refreshViews(async (parent, offset) => {
      asked.push([parent, offset]);
      return parent === 0 ? [withDiff(node(1), 5), withDiff(node(2), 7)] : [withDiff(node(3, false), 9)];
    });
    expect(ok).toBe(true);
    expect(asked.sort()).toEqual([[0, 0], [1, 0]]);
    expect(m.get(2)!.node.diff?.delta).toBe(7);
    expect(m.get(3)!.node.diff?.delta).toBe(9);
    expect(m.get(1)!.expanded).toBe(true);
    expect(m.rows()).toEqual(before);
  });

  it('removes the fields again when the fresh views have none', async () => {
    const m = new TreeModel(node(0));
    m.addPage(0, 0, [withDiff(node(1), 5)], 10);
    await m.refreshViews(async () => [node(1)]);
    expect(m.get(1)!.node.diff).toBeUndefined();
  });

  it('reports a failed page and leaves the others refreshed', async () => {
    const m = new TreeModel(node(0));
    m.addPage(0, 0, [node(1)], 10);
    m.addPage(1, 0, [node(2, false)], 10);
    const ok = await m.refreshViews(async (parent) => {
      if (parent === 1) throw new Error('gone');
      return [withDiff(node(1), 3)];
    });
    expect(ok).toBe(false);
    expect(m.get(1)!.node.diff?.delta).toBe(3);
    expect(m.get(2)!.node.diff).toBeUndefined();
  });

  it('stops applying results once it is no longer alive', async () => {
    const m = new TreeModel(node(0));
    m.addPage(0, 0, [node(1)], 10);
    let alive = true;
    await m.refreshViews(async () => {
      alive = false;
      return [withDiff(node(1), 3)];
    }, () => alive);
    expect(m.get(1)!.node.diff).toBeUndefined();
  });
});
