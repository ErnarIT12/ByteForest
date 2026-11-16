import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('./api', () => ({
  restartElevated: vi.fn(),
  takeStartupScan: vi.fn(),
  listDrives: vi.fn(),
  isElevated: vi.fn(),
  startScan: vi.fn(),
  cancelScan: vi.fn(),
  onScanProgress: vi.fn(),
  onScanDone: vi.fn(),
  onScanError: vi.fn(),
  SCAN_CANCELLED: 'cancelled',
}));

import * as api from './api';
import { listenToScanEvents, loadEnvironment, requestElevation } from './scanController';
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
  for (const f of [api.onScanProgress, api.onScanDone, api.onScanError]) {
    vi.mocked(f as (cb: unknown) => Promise<() => void>)
      .mockReset()
      .mockResolvedValue(() => {});
  }
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

  it('scans a folder as given without touching the drive select', async () => {
    void listenToScanEvents();
    vi.mocked(api.takeStartupScan).mockResolvedValue('D:\\Data\\Stuff');
    loadEnvironment();
    await flush();
    expect(api.startScan).toHaveBeenCalledWith('D:\\Data\\Stuff');
    const s = useAppStore.getState();
    expect(s.target).toBe('C'); // default pick, not a fake "D"
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
