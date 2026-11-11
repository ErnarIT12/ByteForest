import { useEffect, useState } from 'react';
import { formatCount, formatSeconds } from '../format';
import { useAppStore } from '../store';
import { strings } from '../strings';

/** "Parsing · 1.2M files · 3.4 s" for the running scan, with a live clock. */
export function ScanProgressText() {
  const progress = useAppStore((s) => s.progress);
  const startedAt = useAppStore((s) => s.scanStartedAt);
  const [now, setNow] = useState(() => Date.now());

  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 100);
    return () => window.clearInterval(timer);
  }, []);

  const elapsed = startedAt === null ? 0 : now - startedAt;
  const phase = progress ? strings.phase[progress.phase] : strings.starting;
  return (
    // Not a live region: the 100 ms clock would make screen readers chatter.
    // ScanAnnouncer announces the start and the end of a scan instead.
    <span className="progress-text" aria-live="off">
      <span className="spinner" aria-hidden="true" />
      <span className="progress-phase">{phase}</span>
      <span>
        {formatCount(progress?.files ?? 0)} {strings.files} · {formatSeconds(elapsed)}
      </span>
    </span>
  );
}
