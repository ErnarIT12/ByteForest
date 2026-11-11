import type { ScanStats } from './api';
import { strings } from './strings';

/** Label and tooltip of the status-bar badge. */
export interface StatusBadge {
  admin: boolean;
  label: string;
  title: string;
}

/**
 * Describes the badge from the elevation state and the last scan.
 *
 * An MFT scan always ran elevated, so it shows the admin badge even when it took the
 * slower record-by-record path; its fallback reason then becomes the tooltip.
 */
export function statusBadge(elevated: boolean, stats: ScanStats | null): StatusBadge {
  const admin = elevated || stats?.method === 'Mft';
  const fallback = stats?.fallback_reason ?? null;
  let title: string;
  if (fallback !== null && stats?.method === 'Mft') {
    title = strings.mftFallbackHint(fallback);
  } else if (fallback !== null) {
    title = fallback;
  } else {
    title = admin ? strings.adminHint : strings.standardHint;
  }
  return { admin, label: admin ? strings.admin : strings.standard, title };
}
