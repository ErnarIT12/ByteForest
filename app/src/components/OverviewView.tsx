import { useSystemOverview } from '../hooks/useSystemOverview';
import { memoryPercent } from '../overviewModel';
import { strings } from '../strings';
import { OverviewProcessesCard } from './OverviewProcessesCard';
import { OverviewResourcesCard } from './OverviewResourcesCard';
import { OverviewSparklineCard } from './OverviewSparklineCard';

/** Lightweight live system dashboard: resources strip, full-width sparklines, and top processes. */
export function OverviewView() {
  const { data, cpuHistory, memHistory, error } = useSystemOverview();
  const memPct = data ? memoryPercent(data.mem_used_bytes, data.mem_total_bytes) : null;

  return (
    <main className="overview" aria-label={strings.overviewTitle}>
      {error && (
        <p className="overview-error" role="alert">
          {strings.overviewError}: {error}
        </p>
      )}

      <div className="overview-scroll">
        <div className="overview-grid">
          <OverviewResourcesCard data={data} />

          <div className="overview-sparklines">
            <OverviewSparklineCard
              title={strings.overviewCpuUsage}
              currentPct={data ? data.cpu_usage_pct : null}
              samples={cpuHistory}
              tone="accent"
            />
            <OverviewSparklineCard
              title={strings.overviewMemoryUsage}
              currentPct={memPct}
              samples={memHistory}
              tone="silver"
            />
          </div>

          <OverviewProcessesCard
            processes={data?.top_processes ?? []}
            totalMemBytes={data?.mem_total_bytes ?? 0}
          />
        </div>
      </div>
    </main>
  );
}
