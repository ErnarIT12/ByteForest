import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api', () => ({
  listAutoSnapshots: vi.fn(),
  onSnapshotsChanged: vi.fn(),
}));

import * as api from './api';
import { useAutoSnapshots, watchAutoSnapshots } from './autoSnapshots';

const snap = (id: string): api.AutoSnapshot => ({
  id,
  label: 'C:',
  created_unix_ms: 1000,
  node_count: 10,
  bytes_on_disk: 128,
});

const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  useAutoSnapshots.setState({ list: [] });
  vi.mocked(api.listAutoSnapshots).mockReset().mockResolvedValue([]);
  vi.mocked(api.onSnapshotsChanged).mockReset().mockResolvedValue(() => {});
});

describe('watchAutoSnapshots', () => {
  it('unsubscribes on cleanup and ignores events emitted after unmount', async () => {
    const unlisten = vi.fn();
    let emit: (() => void) | undefined;
    let resolveSub: (fn: () => void) => void = () => {};
    vi.mocked(api.onSnapshotsChanged).mockImplementation((cb) => {
      emit = cb;
      return new Promise((r) => {
        resolveSub = r;
      });
    });

    const stop = watchAutoSnapshots();
    expect(api.listAutoSnapshots).toHaveBeenCalledTimes(1);

    stop();
    emit?.();
    expect(api.listAutoSnapshots).toHaveBeenCalledTimes(1);

    resolveSub(unlisten);
    await flush();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it('drops out-of-order older listAutoSnapshots responses', async () => {
    let resolveFirst: (list: api.AutoSnapshot[]) => void = () => {};
    let resolveSecond: (list: api.AutoSnapshot[]) => void = () => {};
    vi.mocked(api.listAutoSnapshots)
      .mockReturnValueOnce(new Promise((r) => { resolveFirst = r; }))
      .mockReturnValueOnce(new Promise((r) => { resolveSecond = r; }));

    const first = useAutoSnapshots.getState().refresh();
    const second = useAutoSnapshots.getState().refresh();

    resolveSecond([snap('newer.bfsnap')]);
    await second;
    expect(useAutoSnapshots.getState().list.map((s) => s.id)).toEqual(['newer.bfsnap']);

    resolveFirst([snap('older.bfsnap')]);
    await first;
    expect(useAutoSnapshots.getState().list.map((s) => s.id)).toEqual(['newer.bfsnap']);
  });
});
