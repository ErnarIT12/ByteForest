import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api', () => ({
  restartElevated: vi.fn(),
  takeStartupScan: vi.fn(),
  listDrives: vi.fn(),
  listUserFolders: vi.fn(),
  isElevated: vi.fn(),
  startScan: vi.fn(),
  cancelScan: vi.fn(),
  pickFolder: vi.fn(),
  onScanProgress: vi.fn(),
  onScanDone: vi.fn(),
  onScanError: vi.fn(),
  SCAN_CANCELLED: 'cancelled',
}));

import * as api from './api';
import { chooseFolderAndScan, listenToScanEvents, loadEnvironment, requestElevation } from './scanController';
import { useAppStore } from './store';

const drive = { letter: 'C', label: '', fs: 'NTFS', total: 100, free: 50 };

/** Lets the promise chains started by `loadEnvironment` settle. */
const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  useAppStore.getState().reset();
  vi.restoreAllMocks();
  vi.mocked(api.restartElevated).mockReset();
  vi.mocked(api.startScan).mockReset().mockResolvedValue(1);
  vi.mocked(api.takeStartupScan).mockReset();
  vi.mocked(api.listDrives).mockReset().mockResolvedValue([drive]);
  vi.mocked(api.isElevated).mockReset().mockResolvedValue(true);
  vi.mocked(api.listUserFolders).mockReset().mockResolvedValue([]);
  for (const f of [api.onScanProgress, api.onScanDone, api.onScanError]) {
    vi.mocked(f as (cb: unknown) => Promise<() => void>)
      .mockReset()
      .mockResolvedValue(() => {});
  }
});

describe('chooseFolderAndScan', () => {
  it('scans the chosen folder and leaves the selection alone on cancel', async () => {
    vi.mocked(api.pickFolder).mockResolvedValueOnce('C:/Demo').mockResolvedValueOnce(null);
    await chooseFolderAndScan();
    await chooseFolderAndScan();
    expect(api.startScan).toHaveBeenCalledExactlyOnceWith('C:/Demo');
    expect(useAppStore.getState().target).toBe('C:/Demo');
  });
});

describe('requestElevation', () => {
  it('ignores a declined prompt silently and restores the button', async () => {
    vi.mocked(api.restartElevated).mockRejectedValue('declined');
    const pending = requestElevation('C');
    expect(useAppStore.getState().elevating).toBe(true);
    await pending;
    expect(api.restartElevated).toHaveBeenCalledWith('C:');
    expect(useAppStore.getState().elevating).toBe(false);
    expect(useAppStore.getState().error).toBeNull();
  });

  it('shows a toast for other failures', async () => {
    vi.mocked(api.restartElevated).mockRejectedValue('spawn failed');
    await requestElevation('C');
    const s = useAppStore.getState();
    expect(s.elevating).toBe(false);
    expect(s.error).toBe('spawn failed');
    expect(s.errorTitle).toBe('Could not restart as administrator');
  });

  it('accepts a full drive root and passes it on unchanged', async () => {
    vi.mocked(api.restartElevated).mockRejectedValue('declined');
    await requestElevation('C:');
    expect(api.restartElevated).toHaveBeenCalledWith('C:');
  });

  it('keeps elevating after success because the app is exiting', async () => {
    vi.mocked(api.restartElevated).mockResolvedValue(undefined);
    await requestElevation('C');
    expect(useAppStore.getState().elevating).toBe(true);
  });

  it('ignores a second request while one is in flight', async () => {
    vi.mocked(api.restartElevated).mockResolvedValue(undefined);
    await Promise.all([requestElevation('C'), requestElevation('C')]);
    expect(api.restartElevated).toHaveBeenCalledTimes(1);
  });
});

describe('listenToScanEvents', () => {
  it('resolves once every listener is registered, then unlistens them all', async () => {
    const unlisten = [vi.fn(), vi.fn(), vi.fn()];
    let release: (f: () => void) => void = () => {};
    vi.mocked(api.onScanProgress).mockReturnValue(Promise.resolve(unlisten[0]));
    vi.mocked(api.onScanDone).mockReturnValue(
      new Promise((r) => {
        release = r;
      }),
    );
    vi.mocked(api.onScanError).mockReturnValue(Promise.resolve(unlisten[2]));

    let settled = false;
    const ready = listenToScanEvents().then((fn) => {
      settled = true;
      return fn;
    });
    await flush();
    expect(settled).toBe(false);
    release(unlisten[1]);
    const stop = await ready;
    expect(settled).toBe(true);
    stop();
    for (const u of unlisten) expect(u).toHaveBeenCalledTimes(1);
  });

  it('unsubscribes and suppresses callbacks if cancelled before registration settles', async () => {
    const unlisten = [vi.fn(), vi.fn(), vi.fn()];
    let progressCb: ((p: api.ScanProgress) => void) | undefined;
    let release: (f: () => void) => void = () => {};
    vi.mocked(api.onScanProgress).mockImplementation((cb) => {
      progressCb = cb;
      return Promise.resolve(unlisten[0]);
    });
    vi.mocked(api.onScanDone).mockReturnValue(
      new Promise((r) => {
        release = r;
      }),
    );
    vi.mocked(api.onScanError).mockReturnValue(Promise.resolve(unlisten[2]));

    const signal = { cancelled: false };
    const ready = listenToScanEvents(signal);
    signal.cancelled = true;
    useAppStore.getState().beginScan('C:');
    progressCb?.({ scan_id: 1, files: 10, dirs: 1, bytes: 100, phase: 'Walking' });
    expect(useAppStore.getState().progress).toBeNull();

    release(unlisten[1]);
    const stop = await ready;
    for (const u of unlisten) expect(u).toHaveBeenCalledTimes(1);
    // Calling stop again is idempotent.
    stop();
    for (const u of unlisten) expect(u).toHaveBeenCalledTimes(1);
  });

  it('cleans up registered listeners even when another listener fails to register', async () => {
    const unlisten0 = vi.fn();
    const unlisten2 = vi.fn();
    vi.mocked(api.onScanProgress).mockResolvedValue(unlisten0);
    vi.mocked(api.onScanDone).mockRejectedValue(new Error('ipc failed'));
    vi.mocked(api.onScanError).mockResolvedValue(unlisten2);

    const stop = await listenToScanEvents();
    stop();
    expect(unlisten0).toHaveBeenCalledTimes(1);
    expect(unlisten2).toHaveBeenCalledTimes(1);
  });

  it('ignores stale scan://done events without refreshing drives', async () => {
    let doneCb: ((d: api.ScanDone) => void) | undefined;
    vi.mocked(api.onScanDone).mockImplementation((cb) => {
      doneCb = cb;
      return Promise.resolve(() => {});
    });
    await listenToScanEvents();
    useAppStore.getState().beginScan('C:');
    useAppStore.getState().scanResolved(2);
    vi.mocked(api.listDrives).mockClear();

    doneCb?.({
      scan_id: 1,
      stats: { files: 1, dirs: 0, bytes: 10, elapsed_ms: 1, method: 'Walk', fallback_reason: null, skipped_records: 0, errors: 0 },
      root: { id: 0, name: 'C:', size: 10, alloc: 10, files: 1, dirs: 0, is_dir: true, has_children: false, mtime: 0, percent_of_parent: 0 },
    });
    expect(api.listDrives).not.toHaveBeenCalled();
  });
});

describe('startup scan', () => {
  it('waits for the listeners before calling startScan', async () => {
    let release: (f: () => void) => void = () => {};
    vi.mocked(api.onScanProgress).mockReturnValue(
      new Promise((r) => {
        release = r;
      }),
    );
    void listenToScanEvents();
    vi.mocked(api.takeStartupScan).mockResolvedValue('C:\\');
    loadEnvironment();
    await flush();
    expect(api.startScan).not.toHaveBeenCalled();
    release(() => {});
    await flush();
    expect(api.startScan).toHaveBeenCalledWith('C:');
    expect(useAppStore.getState().target).toBe('C');
  });

  it('selects the requested folder so refresh scans the same path', async () => {
    void listenToScanEvents();
    vi.mocked(api.takeStartupScan).mockResolvedValue('D:\\Data\\Stuff');
    loadEnvironment();
    await flush();
    expect(api.startScan).toHaveBeenCalledWith('D:\\Data\\Stuff');
    const s = useAppStore.getState();
    expect(s.target).toBe(s.scanTarget);
    expect(s.scanTarget).toBe('D:\\Data\\Stuff');
  });

  it('ignores values that are neither a drive nor an absolute path', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    void listenToScanEvents();
    vi.mocked(api.takeStartupScan).mockResolvedValue('relative\\dir');
    loadEnvironment();
    await flush();
    expect(api.startScan).not.toHaveBeenCalled();
    expect(warn).toHaveBeenCalled();
  });
});
