import { describe, expect, it } from 'vitest';
import type { ScanStats, VolumeSummary } from './api';
import { canOfferFastScan, driveLetterOf, isDeclined, shouldShowElevationBar } from './elevation';

const vol = (letter: string, fs: string): VolumeSummary => ({
  letter,
  label: '',
  fs,
  total: 100,
  free: 50,
});
const drives = [vol('C', 'NTFS'), vol('F', 'FAT32')];
const stats = (method: 'Mft' | 'Walk'): ScanStats => ({
  files: 1,
  dirs: 1,
  bytes: 1,
  elapsed_ms: 58300,
  method,
  fallback_reason: null,
  skipped_records: 0,
  errors: 0,
});

describe('canOfferFastScan', () => {
  it('offers for an NTFS drive root when not elevated', () => {
    expect(canOfferFastScan(false, 'C:', drives)).toBe(true);
    expect(canOfferFastScan(false, 'C', drives)).toBe(true);
  });
  it('not when elevated, for folders or for non-NTFS drives', () => {
    expect(canOfferFastScan(true, 'C:', drives)).toBe(false);
    expect(canOfferFastScan(false, 'E:\\Folder', drives)).toBe(false);
    expect(canOfferFastScan(false, 'F:', drives)).toBe(false);
    expect(canOfferFastScan(false, 'Z:', drives)).toBe(false);
  });
});

describe('shouldShowElevationBar', () => {
  it('shows after a walked NTFS scan', () => {
    expect(shouldShowElevationBar(false, stats('Walk'), 'C', drives, false)).toBe(true);
  });
  it('hides for MFT scans, dismissal, elevation, missing stats and non-NTFS', () => {
    expect(shouldShowElevationBar(false, stats('Mft'), 'C', drives, false)).toBe(false);
    expect(shouldShowElevationBar(false, stats('Walk'), 'C', drives, true)).toBe(false);
    expect(shouldShowElevationBar(true, stats('Walk'), 'C', drives, false)).toBe(false);
    expect(shouldShowElevationBar(false, null, 'C', drives, false)).toBe(false);
    expect(shouldShowElevationBar(false, stats('Walk'), 'F', drives, false)).toBe(false);
  });
});

describe('helpers', () => {
  it('parses drive roots only', () => {
    expect(driveLetterOf('c:\\')).toBe('C');
    expect(driveLetterOf('C:\\Users')).toBeNull();
  });
  it('recognises a declined UAC prompt', () => {
    expect(isDeclined('declined')).toBe(true);
    expect(isDeclined(new Error('declined'))).toBe(true);
    expect(isDeclined('spawn failed')).toBe(false);
  });
});
