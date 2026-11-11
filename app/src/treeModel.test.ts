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
