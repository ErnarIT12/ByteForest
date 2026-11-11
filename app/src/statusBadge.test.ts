import { describe, expect, it } from 'vitest';
import type { ScanStats } from './api';
import { statusBadge } from './statusBadge';
import { strings } from './strings';

const stats = (over: Partial<ScanStats>): ScanStats => ({
  files: 1,
  dirs: 1,
  bytes: 1,
  elapsed_ms: 1,
  method: 'Mft',
  fallback_reason: null,
  skipped_records: 0,
  errors: 0,
  ...over,
});

describe('statusBadge', () => {
  it('shows the admin badge before any scan', () => {
    expect(statusBadge(true, null)).toEqual({
      admin: true,
      label: strings.admin,
      title: strings.adminHint,
    });
  });

  it('shows standard mode without elevation', () => {
    const reason = 'administrator rights required for fast scan';
    const badge = statusBadge(false, stats({ method: 'Walk', fallback_reason: reason }));
    expect(badge.label).toBe(strings.standard);
    expect(badge.title).toBe(reason);
  });

  it('keeps the admin badge for an MFT scan that read records one by one', () => {
    const reason = 'raw MFT reads blocked (access denied); read records one by one';
    const badge = statusBadge(true, stats({ fallback_reason: reason }));
    expect(badge.admin).toBe(true);
    expect(badge.label).toBe(strings.admin);
    expect(badge.label).not.toBe(strings.standard);
    expect(badge.title).toContain(reason);
  });

  it('shows the walk fallback reason when elevated', () => {
    const reason = 'filesystem is FAT32, not NTFS';
    const badge = statusBadge(true, stats({ method: 'Walk', fallback_reason: reason }));
    expect(badge.label).toBe(strings.admin);
    expect(badge.title).toBe(reason);
  });
});
