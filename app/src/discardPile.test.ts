import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { BatchTrashResult, DiscardSummary, NodeView, ScanDone } from './api';

const api = vi.hoisted(() => ({
  discardSummary: vi.fn(),
  moveNodesToTrash: vi.fn(),
  locateNode: vi.fn(),
}));
vi.mock('./api', async (importOriginal) => ({ ...(await importOriginal<object>()), ...api }));

import { useDiscardPile } from './discardPile';
import { useAppStore } from './store';

const root: NodeView = {
  id: 0,
  name: 'C:',
  size: 1000,
  alloc: 1000,
  files: 10,
  dirs: 3,
  is_dir: true,
  has_children: true,
  mtime: 0,
  percent_of_parent: 0,
};
const done: ScanDone = {
  scan_id: 1,
  root,
  stats: {
    files: 10,
    dirs: 3,
    bytes: 1000,
    elapsed_ms: 1,
    method: 'Walk',
    fallback_reason: null,
    skipped_records: 0,
    errors: 0,
  },
};

const summaryOf = (...ids: number[]): DiscardSummary => ({
  items: ids.map((id) => ({ id, name: `n${id}`, path: `C:\\n${id}`, size: 100, is_dir: false })),
  count: ids.length,
  bytes: ids.length * 100,
});

/** Lets pending promise callbacks run. */
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

beforeEach(() => {
  vi.resetAllMocks();
  useAppStore.getState().reset();
  const app = useAppStore.getState();
  app.beginScan('C');
  app.scanResolved(1);
  useAppStore.getState().handleDone(done);
  useDiscardPile.getState().bindScan(null);
  useDiscardPile.getState().bindScan(1);
  api.discardSummary.mockImplementation((_scan: number, nodes: number[]) =>
    Promise.resolve(summaryOf(...nodes)),
  );
  api.locateNode.mockResolvedValue([]);
});

describe('Discard Pile queue', () => {
  it('queues each id once and never the root', async () => {
    useDiscardPile.getState().add([4, 5, 4, 0]);
    await settle();
    const pile = useDiscardPile.getState();
    expect(pile.ids).toEqual([4, 5]);
    expect(pile.has(4)).toBe(true);
    expect(pile.has(0)).toBe(false);
    expect(pile.summary?.count).toBe(2);
  });

  it('removes and clears items', async () => {
    useDiscardPile.getState().add([4, 5, 6]);
    useDiscardPile.getState().remove(5);
    expect(useDiscardPile.getState().ids).toEqual([4, 6]);
    useDiscardPile.getState().clear();
    await settle();
    expect(useDiscardPile.getState().ids).toEqual([]);
    expect(useDiscardPile.getState().summary).toBeNull();
  });

  it('prunes ids the backend no longer lists, such as nested items', async () => {
    api.discardSummary.mockResolvedValue(summaryOf(4));
    useDiscardPile.getState().add([4, 5]);
    await settle();
    expect(useDiscardPile.getState().ids).toEqual([4]);
    expect(useDiscardPile.getState().idSet.has(5)).toBe(false);
  });

  it('starts empty for another scan result', () => {
    useDiscardPile.getState().add([4]);
    useDiscardPile.getState().bindScan(2);
    expect(useDiscardPile.getState().ids).toEqual([]);
    expect(useDiscardPile.getState().scanId).toBe(2);
  });

  it('ignores adds while no result is bound', () => {
    useDiscardPile.getState().bindScan(null);
    useDiscardPile.getState().add([4]);
    expect(useDiscardPile.getState().ids).toEqual([]);
  });
});

describe('Discard Pile batch move', () => {
  const result: BatchTrashResult = {
    moved: [
      { id: 4, bytes: 100, files: 1, dirs: 0, parent: 0 },
      { id: 5, bytes: 200, files: 2, dirs: 0, parent: 0 },
    ],
    failed: [{ id: 6, name: 'n6', error: 'in use' }],
    bytes: 300,
    files: 3,
    dirs: 0,
  };

  it('removes moved items, keeps failed ones queued and updates the totals once', async () => {
    useDiscardPile.getState().add([4, 5, 6]);
    await settle();
    // After the batch the backend only knows the item that failed.
    api.moveNodesToTrash.mockImplementation(() => {
      api.discardSummary.mockResolvedValue(summaryOf(6));
      return Promise.resolve(result);
    });
    const before = useAppStore.getState().generation;
    useDiscardPile.getState().requestMoveAll();
    expect(useDiscardPile.getState().confirming).toBe(true);
    await useDiscardPile.getState().confirmMoveAll();
    await settle();

    expect(api.moveNodesToTrash).toHaveBeenCalledWith(1, [4, 5, 6]);
    const pile = useDiscardPile.getState();
    expect(pile.ids).toEqual([6]);
    expect(pile.busy).toBe(false);
    expect(pile.confirming).toBe(false);
    expect(pile.outcome).toMatchObject({ moved: 2, bytes: 300, error: null });
    expect(pile.outcome?.failed.map((f) => f.id)).toEqual([6]);
    const app = useAppStore.getState();
    expect(app.root).toMatchObject({ size: 700, files: 7 });
    expect(app.generation).toBe(before + 1);
  });

  it('reports a failed batch and keeps every item queued', async () => {
    useDiscardPile.getState().add([4]);
    await settle();
    api.moveNodesToTrash.mockRejectedValue('A scan is in progress or the results have changed.');
    useDiscardPile.getState().requestMoveAll();
    await useDiscardPile.getState().confirmMoveAll();
    const pile = useDiscardPile.getState();
    expect(pile.ids).toEqual([4]);
    expect(pile.outcome?.error).toContain('scan is in progress');
    expect(pile.busy).toBe(false);
  });

  it('does not ask to move an empty pile', () => {
    useDiscardPile.getState().requestMoveAll();
    expect(useDiscardPile.getState().confirming).toBe(false);
  });
});
