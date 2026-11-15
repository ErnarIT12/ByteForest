import type { ScanStats, VolumeSummary } from './api';

/** Drive letter of a drive-root target ("C", "C:" or "C:\\"), or null for anything else. */
export function driveLetterOf(target: string): string | null {
  const m = /^([A-Za-z]):?\\?$/.exec(target.trim());
  return m ? m[1].toUpperCase() : null;
}

/** True when `target` is the root of an NTFS drive in `drives`. */
function isNtfsDrive(target: string, drives: VolumeSummary[]): boolean {
  const letter = driveLetterOf(target);
  if (letter === null) return false;
  return drives.some((d) => d.letter.toUpperCase() === letter && d.fs.toUpperCase() === 'NTFS');
}

/** Whether the "Fast scan (admin)" button makes sense: not elevated yet and an NTFS drive root. */
export function canOfferFastScan(
  elevated: boolean,
  target: string,
  drives: VolumeSummary[],
): boolean {
  return !elevated && isNtfsDrive(target, drives);
}

/** Whether to show the "restart as administrator" hint after a slow (walked) scan. */
export function shouldShowElevationBar(
  elevated: boolean,
  stats: ScanStats | null,
  target: string,
  drives: VolumeSummary[],
  dismissed: boolean,
): boolean {
  return !dismissed && !elevated && stats?.method === 'Walk' && isNtfsDrive(target, drives);
}

/** True when `restartElevated` was refused by the user in the UAC prompt. */
export function isDeclined(error: unknown): boolean {
  return String(error).trim().replace(/^Error:\s*/, '') === 'declined';
}
