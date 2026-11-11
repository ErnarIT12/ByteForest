import { formatInt, formatSeconds } from '../format';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** Visually hidden live region: announces when a scan starts and how it ended. */
export function ScanAnnouncer() {
  const scanning = useAppStore((s) => s.scanning);
  const scanTarget = useAppStore((s) => s.scanTarget);
  const outcome = useAppStore((s) => s.lastOutcome);
  const stats = useAppStore((s) => s.stats);

  let text = '';
  if (scanning) text = strings.announceStart(`${scanTarget ?? ''}:`);
  else if (outcome === 'done' && stats) {
    text = strings.announceDone(formatInt(stats.files), formatSeconds(stats.elapsed_ms));
  } else if (outcome === 'cancelled') text = strings.announceCancelled;
  else if (outcome === 'failed') text = strings.announceFailed;

  return (
    <div className="sr-only" role="status" aria-live="polite">
      {text}
    </div>
  );
}
