import { driveLetterOf } from './elevation';

/**
 * What `startScan` receives for the location picked in the UI: a drive letter ("C") becomes a
 * drive root ("C:"), anything else (a folder path) is used as it is.
 */
export function scanTargetFor(target: string): string {
  return /^[A-Za-z]$/.test(target) ? `${target.toUpperCase()}:` : target;
}

/** The location (sidebar selection) a finished scan's target corresponds to: "C:" gives "C". */
export function locationOf(scanTarget: string): string {
  return driveLetterOf(scanTarget) ?? scanTarget;
}

/** The last path segment of a folder path ("C:\Users\me\Desktop" gives "Desktop"). */
export function folderLabel(path: string): string {
  const parts = path.split(/[\\/]+/).filter((part) => part !== '');
  return parts.length > 0 ? parts[parts.length - 1] : path;
}
