import type { SystemOverview, VolumeSummary } from '../api';
import { formatBytes, formatInt } from '../format';
import { drivePercent, formatWholePercent, memoryPercent } from '../overviewModel';
import { runScan } from '../scanController';
import { folderLabel, scanTargetFor } from '../scanTargets';
import { useAppStore } from '../store';
import { strings } from '../strings';

const RING_RADIUS = 23;
const RING_CIRCUMFERENCE = 2 * Math.PI * RING_RADIUS;

function MiniRing({
  percent,
  label,
  tone = 'accent',
}: {
  percent: number;
  label: string;
  tone?: 'accent' | 'silver' | 'warm';
}) {
  const clamped = Math.min(100, Math.max(0, percent));
  const offset = RING_CIRCUMFERENCE * (1 - clamped / 100);
  return (
    <div className={`overview-ring tone-${tone}`} aria-hidden="true">
      <svg viewBox="0 0 60 60">
        <circle className="overview-ring-track" cx="30" cy="30" r={RING_RADIUS} />
        <circle
          className="overview-ring-fill"
          cx="30"
          cy="30"
          r={RING_RADIUS}
          strokeDasharray={RING_CIRCUMFERENCE}
          strokeDashoffset={offset}
        />
      </svg>
      <span>{label}</span>
    </div>
  );
}

function driveTitle(drive: VolumeSummary): string {
  if (drive.path) return drive.label || folderLabel(drive.path);
  return [drive.letter ? `${drive.letter}:` : '', drive.label].filter(Boolean).join(' ');
}

function driveValue(drive: VolumeSummary): string {
  return drive.path ?? drive.letter ?? '';
}

/** Compact resources strip: mini radial rings for CPU, Memory, and clickable Drives. */
export function OverviewResourcesCard({ data }: { data: SystemOverview | null }) {
  const drives = useAppStore((s) => s.drives);
  const scanning = useAppStore((s) => s.scanning);

  const cpuPct = data?.cpu_usage_pct ?? 0;
  const memPct = data ? memoryPercent(data.mem_used_bytes, data.mem_total_bytes) : 0;
  const storageDrives = (drives ?? []).filter((d) => d.total > 0);

  const scanDrive = (drive: VolumeSummary) => {
    const value = driveValue(drive);
    if (!value) return;
    useAppStore.getState().setTarget(value);
    void runScan(scanTargetFor(value));
  };

  return (
    <section className="overview-card overview-resources" aria-label={strings.overviewResources}>
      <div className="overview-ring-grid">
        <div className="resource-tile">
          <MiniRing percent={cpuPct} label={data ? formatWholePercent(cpuPct) : '—'} tone="accent" />
          <div className="resource-meta">
            <strong>{strings.overviewCpu}</strong>
            <span>
              {data ? strings.overviewCpuCores(formatInt(data.cpu_cores)) : strings.overviewSampling}
            </span>
          </div>
        </div>

        <div className="resource-tile">
          <MiniRing percent={memPct} label={data ? formatWholePercent(memPct) : '—'} tone="silver" />
          <div className="resource-meta">
            <strong>{strings.overviewMemory}</strong>
            <span>
              {data
                ? strings.overviewMemoryOf(formatBytes(data.mem_used_bytes), formatBytes(data.mem_total_bytes))
                : strings.overviewSampling}
            </span>
          </div>
        </div>

        {storageDrives.map((drive) => {
          const name = driveTitle(drive);
          const pct = drivePercent(drive.free, drive.total);
          const tone = pct >= 88 ? 'warm' : 'accent';
          return (
            <button
              key={driveValue(drive)}
              type="button"
              className="resource-tile resource-drive"
              disabled={scanning}
              aria-label={strings.overviewScanDrive(name)}
              title={`${strings.overviewScanDrive(name)} (${strings.overviewDriveFree(formatBytes(drive.free))})`}
              onClick={() => scanDrive(drive)}
            >
              <MiniRing percent={pct} label={formatWholePercent(pct)} tone={tone} />
              <div className="resource-meta">
                <strong>{name}</strong>
                <span>{strings.overviewDriveFree(formatBytes(drive.free))}</span>
              </div>
            </button>
          );
        })}
      </div>
    </section>
  );
}
