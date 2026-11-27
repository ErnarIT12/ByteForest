import { formatBytes, formatInt } from '../format';
import { useSystemOverview } from '../hooks/useSystemOverview';
import { memoryPercent } from '../overviewModel';
import { chooseFolderAndScan } from '../scanController';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { FastScanButton } from './FastScanButton';
import { FolderOpen, Icon } from './Icon';
import { OverviewProcessesCard } from './OverviewProcessesCard';
import { OverviewResourcesCard } from './OverviewResourcesCard';
import { OverviewSparklineCard } from './OverviewSparklineCard';
import { ScanButton } from './ScanButton';

/** Lightweight live system dashboard: resources strip, CPU/RAM sparklines, and top processes. */
export function OverviewView() {
  const { data, cpuHistory, memHistory, error } = useSystemOverview();
  const target = useAppStore((s) => s.target);
  const drives = useAppStore((s) => s.drives);

  const activeDrive = drives?.find((d) => (d.path ?? d.letter) === target);
  const scanLabel = activeDrive?.total ? 'Scan drive' : 'Scan folder';

  const headerSubtitle = data
    ? [
        data.cpu_brand,
        strings.overviewCpuCores(formatInt(data.cpu_cores)),
        data.mem_total_bytes > 0 ? `${formatBytes(data.mem_total_bytes)} RAM` : '',
      ]
        .filter(Boolean)
        .join(' · ')
    : strings.overviewSubtitle;
  const cpuSubtitle = data
    ? strings.overviewCpuCores(formatInt(data.cpu_cores))
    : strings.overviewSampling;
  const memSubtitle = data
    ? strings.overviewMemoryOf(formatBytes(data.mem_used_bytes), formatBytes(data.mem_total_bytes))
    : strings.overviewSampling;
  const memPct = data ? memoryPercent(data.mem_used_bytes, data.mem_total_bytes) : null;

  return (
    <main className="overview" aria-label={strings.overviewTitle}>
      <header className="overview-heading">
        <div className="overview-title-block">
          <h1>{strings.overviewTitle}</h1>
          <p>{headerSubtitle}</p>
        </div>
        <div className="overview-actions">
          {target !== null && <ScanButton label={scanLabel} />}
          <button
            type="button"
            className={target === null ? 'btn primary' : 'btn'}
            onClick={() => void chooseFolderAndScan()}
          >
            <Icon icon={FolderOpen} size={15} />
            {strings.chooseFolder}
          </button>
          <FastScanButton />
        </div>
      </header>

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
              subtitle={cpuSubtitle}
              currentPct={data ? data.cpu_usage_pct : null}
              samples={cpuHistory}
              tone="accent"
            />
            <OverviewSparklineCard
              title={strings.overviewMemoryUsage}
              subtitle={memSubtitle}
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
