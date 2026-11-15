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
import { requestElevation } from './scanController';
import { useAppStore } from './store';

beforeEach(() => {
  useAppStore.getState().reset();
  vi.mocked(api.restartElevated).mockReset();
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
});
