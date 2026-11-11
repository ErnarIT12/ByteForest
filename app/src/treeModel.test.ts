import { describe, expect, it } from 'vitest';
import type { NodeView } from './api';
import { TreeModel } from './treeModel';

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
