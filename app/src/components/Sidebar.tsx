import { pickFolder, type UserFolder, type VolumeSummary } from '../api';
import { formatBytes } from '../format';
import { setSidebarCollapsed, useSidebarCollapsed } from '../hooks/useSidebarCollapsed';
import { useMediaQuery } from '../hooks/useMediaQuery';
import { runScan } from '../scanController';
import { folderLabel, locationOf, scanTargetFor } from '../scanTargets';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { DiscardPileDock } from './DiscardPileDock';
import {
  Download,
  FileText,
  Folder,
  FolderOpen,
  HardDrive,
  House,
  Icon,
  Monitor,
  PanelLeftClose,
  PanelLeftOpen,
  type LucideIcon,
} from './Icon';
import { RecentScans, type RecentItem } from './RecentScans';

/** Windows narrower than this always show the icon rail so the explorer keeps its room. */
export const SIDEBAR_NARROW_QUERY = '(max-width: 999px)';

interface Location {
  /** Value stored as the target: a drive letter or a folder path. */
  value: string;
  name: string;
  /** Second line (free space); empty for folders. */
  detail: string;
  title: string;
  icon: LucideIcon;
  /** Share of the volume in use (0..1), or null when unknown. */
  used: number | null;
}

const FOLDER_ICONS: Record<string, LucideIcon> = {
  desktop: Monitor,
  downloads: Download,
  documents: FileText,
};

function driveLocation(drive: VolumeSummary): Location {
  const isHome = drive.path !== undefined && drive.fs === '' && drive.total <= 0;
  const name = drive.path
    ? drive.label || folderLabel(drive.path)
    : [drive.letter + ':', drive.label].filter(Boolean).join(' ');
  return {
    value: drive.path ?? drive.letter ?? '',
    name,
    detail: drive.total > 0 ? strings.driveFree(formatBytes(drive.free)) : '',
    title: drive.path ?? name,
    icon: isHome ? House : HardDrive,
    used: drive.total > 0 ? Math.max(0, Math.min(1, 1 - drive.free / drive.total)) : null,
  };
}

function folderLocation(folder: UserFolder): Location {
  return {
    value: folder.path,
    name: strings.userFolderNames[folder.kind as keyof typeof strings.userFolderNames] ?? folder.name,
    detail: '',
    title: folder.path,
    icon: FOLDER_ICONS[folder.kind] ?? Folder,
    used: null,
  };
}

function formatScanTime(finishedAt: number, now = Date.now()): string {
  const when = new Date(finishedAt);
  return new Date(now).toDateString() === when.toDateString()
    ? when.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
    : when.toLocaleDateString([], { month: 'short', day: 'numeric' });
}

function LocationList({ locations, target, onChoose }: {
  locations: Location[];
  target: string | null;
  onChoose(value: string): void;
}) {
  return (
    <ul className="sidebar-list">
      {locations.map((location) => {
        const active = location.value === target;
        return (
          <li key={location.value}>
            <button type="button" className="sidebar-item" title={location.title}
              aria-current={active ? 'true' : undefined} onClick={() => onChoose(location.value)}>
              <Icon icon={location.icon} className="sidebar-icon" />
              <span className="sidebar-text">
                <span className="sidebar-name">{location.name}</span>
                {location.detail && <span className="sidebar-detail">{location.detail}</span>}
                {location.used !== null && (
                  <span className="sidebar-meter" role="img"
                    aria-label={strings.driveUsageLabel(`${Math.round(location.used * 100)}%`)}>
                    <span style={{ width: `${100 * location.used}%` }} />
                  </span>
                )}
              </span>
            </button>
          </li>
        );
      })}
    </ul>
  );
}

/** Left column in every state: where to scan, what was scanned recently and the Discard Pile. */
export function Sidebar() {
  const drives = useAppStore((s) => s.drives);
  const userFolders = useAppStore((s) => s.userFolders);
  const recentScans = useAppStore((s) => s.recentScans);
  const target = useAppStore((s) => s.target);
  const setTarget = useAppStore((s) => s.setTarget);
  const isMac = useAppStore((s) => s.isMac);
  const collapsed = useSidebarCollapsed();
  const narrow = useMediaQuery(SIDEBAR_NARROW_QUERY);
  const rail = collapsed || narrow;

  const choose = (value: string) => {
    setTarget(value);
    void runScan(scanTargetFor(value));
  };

  const chooseFolder = () => {
    void pickFolder().then(
      (folder) => { if (folder) choose(folder); },
      (error: unknown) =>
        useAppStore.setState({ error: String(error), errorTitle: strings.chooseFolderFailed }),
    );
  };

  const locations: Location[] = [
    ...(drives ?? []).map(driveLocation),
    ...userFolders.map(folderLocation),
  ];
  if (target && !locations.some((location) => location.value === target)) {
    locations.push({
      value: target,
      name: folderLabel(target),
      detail: '',
      title: target,
      icon: FolderOpen,
      used: null,
    });
  }

  const recent: RecentItem[] = recentScans.map((scan) => ({
    id: scan.target,
    label: scan.label,
    detail: `${formatBytes(scan.bytes)} · ${formatScanTime(scan.finishedAt)}`,
    title: scan.target,
    onSelect: () => choose(locationOf(scan.target)),
  }));

  return (
    <nav className="sidebar" aria-label={strings.sidebarLabel} data-rail={rail ? 'true' : undefined}>
      {!narrow && (
        <div className="sidebar-head">
          <button type="button" className="icon-btn" aria-expanded={!collapsed}
            aria-controls="sidebar-body"
            aria-label={collapsed ? strings.sidebarExpand : strings.sidebarCollapse}
            title={collapsed ? strings.sidebarExpand : strings.sidebarCollapse}
            onClick={() => setSidebarCollapsed(!collapsed)}>
            <Icon icon={collapsed ? PanelLeftOpen : PanelLeftClose} />
          </button>
        </div>
      )}
      <div id="sidebar-body" className="sidebar-body">
        <section className="sidebar-section" aria-labelledby="sidebar-locations">
          <h2 id="sidebar-locations" className="sidebar-heading">{strings.locationsTitle}</h2>
          {drives === null ? (
            <p className="sidebar-note">{strings.loadingDrives}</p>
          ) : locations.length === 0 && !isMac ? (
            <p className="sidebar-note">{strings.noDrives}</p>
          ) : (
            <LocationList locations={locations} target={target} onChoose={choose} />
          )}
          {isMac && (
            <button type="button" className="sidebar-item sidebar-action" title={strings.chooseFolder}
              onClick={chooseFolder}>
              <Icon icon={FolderOpen} className="sidebar-icon" />
              <span className="sidebar-text">
                <span className="sidebar-name">{strings.chooseFolder}</span>
              </span>
            </button>
          )}
        </section>
        <RecentScans items={recent} />
      </div>
      <DiscardPileDock rail={rail} />
    </nav>
  );
}
