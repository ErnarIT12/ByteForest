import type { ProcessInfo } from '../api';
import { formatBytes } from '../format';
import { strings } from '../strings';

/** Clean horizontal bar rows for the top 6–8 processes by memory usage. */
export function OverviewProcessesCard({
  processes,
  totalMemBytes,
}: {
  processes: readonly ProcessInfo[];
  totalMemBytes: number;
}) {
  const maxBytes = processes.reduce((max, p) => Math.max(max, p.memory_bytes), 0);
  const scaleDenom = maxBytes > 0 ? maxBytes : Math.max(1, totalMemBytes);

  return (
    <section className="overview-card overview-processes" aria-label={strings.overviewTopProcesses}>
      <header className="overview-card-head">
        <h2>{strings.overviewTopProcesses}</h2>
      </header>

      {processes.length === 0 ? (
        <p className="overview-empty">{strings.overviewProcessesEmpty}</p>
      ) : (
        <ul className="process-list">
          {processes.map((proc) => {
            const widthPct = Math.min(100, Math.max(2, (proc.memory_bytes * 100) / scaleDenom));
            return (
              <li
                key={proc.pid}
                className="process-row"
                title={`${proc.name} · PID ${proc.pid} · ${proc.cpu_pct.toFixed(1)}% CPU`}
              >
                <span className="process-name">{proc.name}</span>
                <span className="process-bar" aria-hidden="true">
                  <span style={{ width: `${widthPct}%` }} />
                </span>
                <span className="process-mem">{formatBytes(proc.memory_bytes)}</span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
