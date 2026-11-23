import { describe, expect, it } from 'vitest';
import { filetimeToDate, formatBytes, formatCount, formatFiletime, formatScanShare, formatSeconds } from './format';

describe('formatBytes', () => {
  it('formats the brief values', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatBytes(1023)).toBe('1023 B');
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(1_073_741_824)).toBe('1.0 GB');
  });

  it('promotes when the rounded value reaches 1024', () => {
    // 1 048 524 / 1024 = 1023.95 KB rounds to 1024 KB: shown as the next unit.
    expect(formatBytes(1_048_524)).toBe('1.0 MB');
    expect(formatBytes(1_048_550)).toBe('1.0 MB');
    // 1023.4 KB stays in KB.
    expect(formatBytes(1023.4 * 1024)).toBe('1023.4 KB');
    expect(formatBytes(1024)).toBe('1.0 KB');
    expect(formatBytes(1_048_575)).toBe('1.0 MB');
  });

  it('covers terabytes and custom precision', () => {
    expect(formatBytes(2.5 * 1024 ** 4)).toBe('2.5 TB');
    expect(formatBytes(476.9 * 1024 ** 3, 0)).toBe('477 GB');
  });
});

describe('formatScanShare', () => {
  it('distinguishes tiny files, empty files, and an empty scan', () => {
    expect(formatScanShare(1, 1_000_000)).toBe('<0.01%');
    expect(formatScanShare(0, 100)).toBe('0%');
    expect(formatScanShare(0, 0)).toBe('—');
    expect(formatScanShare(1, 3)).toBe('33.33%');
  });
});

describe('formatCount', () => {
  it('abbreviates thousands and millions', () => {
    expect(formatCount(999)).toBe('999');
    expect(formatCount(12_345)).toBe('12.3K');
    expect(formatCount(1_234_567)).toBe('1.2M');
    expect(formatCount(999_960)).toBe('1.0M');
  });
});

describe('filetimeToDate', () => {
  it('maps the Unix epoch and treats 0 as missing', () => {
    expect(filetimeToDate(116_444_736_000_000_000)?.toISOString()).toBe('1970-01-01T00:00:00.000Z');
    expect(filetimeToDate(0)).toBeNull();
  });
});

describe('formatFiletime', () => {
  it('uses a fixed English short date', () => {
    // 2026-11-15T12:00:00Z as FILETIME; noon keeps the day stable across time zones.
    const ft = Date.UTC(2026, 10, 15, 12) * 10_000 + 116_444_736_000_000_000;
    expect(formatFiletime(ft)).toBe('Nov 15, 2026');
    expect(formatFiletime(0)).toBe('');
  });
});

describe('formatSeconds', () => {
  it('shows one decimal and "<0.1 s" for instant scans', () => {
    expect(formatSeconds(3420)).toBe('3.4 s');
    expect(formatSeconds(0)).toBe('<0.1 s');
    expect(formatSeconds(49)).toBe('<0.1 s');
    expect(formatSeconds(50)).toBe('0.1 s');
    expect(formatSeconds(-5)).toBe('<0.1 s');
  });
});
