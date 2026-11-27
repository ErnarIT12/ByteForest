import { useEffect, useMemo } from 'react';
import { type UserFolder, type VolumeSummary } from '../api';
import { formatBytes } from '../format';
import { setSidebarCollapsed, useSidebarCollapsed } from '../hooks/useSidebarCollapsed';
import { useMediaQuery } from '../hooks/useMediaQuery';
import { watchAutoSnapshots } from '../autoSnapshots';
import { chooseFolderAndScan, runScan } from '../scanController';
import { folderLabel, scanTargetFor } from '../scanTargets';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { useDiscardSync } from './DiscardPileDock';
import {
  Activity,
  Copy,
  Trash2,
  History,
  LayoutGrid,
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
import { SettingsPopover } from './SettingsPopover';
import { RecentScans } from './RecentScans';
import { TreeGlyph } from './TreeGlyph';

/** Windows narrower than this always show the icon rail so the explorer keeps its room. */
export const SIDEBAR_NARROW_QUERY = '(max-width: 999px)';

const SCAN_VIEWS = [
  { label: 'Disk map', icon: LayoutGrid, viewId: 'map' },
  { label: 'Largest files', icon: FileText, viewId: 'files' },
  { label: strings.dupesTitle, icon: Copy, viewId: 'dupes' },
  { label: 'Scan history', icon: History, viewId: 'history' },
  { label: 'Cleanup', icon: Trash2, viewId: 'cleanup' },
] as const;

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

function DiscardSync() {
  useDiscardSync();
  return null;
}

function ScanViews() {
  const hasScan = useAppStore((s) => s.root !== null);
  const view = useAppStore((s) => s.resultView);
  const onOverview = view === 'overview' || !hasScan;
  return (
    <section className="sidebar-section" aria-label="Workspace views">
      <button
        type="button"
        className="sidebar-item"
        title={strings.overviewTitle}
        aria-current={onOverview ? 'page' : undefined}
        onClick={() => useAppStore.setState({ resultView: 'overview' })}
      >
        <Icon icon={Activity} className="sidebar-icon" />
        <span className="sidebar-text">{strings.overviewTitle}</span>
      </button>
      {hasScan &&
        SCAN_VIEWS.map(({ label, icon, viewId }) => (
          <button
            key={label}
            type="button"
            className="sidebar-item"
            title={label}
            aria-current={view === viewId ? 'page' : undefined}
            onClick={() => useAppStore.setState({ resultView: viewId })}
          >
            <Icon icon={icon} className="sidebar-icon" />
            <span className="sidebar-text">{label}</span>
          </button>
        ))}
    </section>
  );
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

/** Full-height left sidebar: brand header, workspace views, locations, recent scans and Settings. */
export function Sidebar() {
  const hasScan = useAppStore((s) => s.root !== null);
  const drives = useAppStore((s) => s.drives);
  const userFolders = useAppStore((s) => s.userFolders);
  const target = useAppStore((s) => s.target);
  const isMac = useAppStore((s) => s.isMac);
  const collapsed = useSidebarCollapsed();
  const narrow = useMediaQuery(SIDEBAR_NARROW_QUERY);
  const rail = collapsed || narrow;

  useEffect(() => watchAutoSnapshots(), []);

  const choose = (value: string) => {
    useAppStore.getState().setTarget(value);
    void runScan(scanTargetFor(value));
  };

  const locations = useMemo<Location[]>(() => {
    const items: Location[] = [
      ...(drives ?? []).map(driveLocation),
      ...userFolders.map(folderLocation),
    ];
    if (target && !items.some((location) => location.value === target)) {
      items.push({
        value: target,
        name: folderLabel(target),
        detail: '',
        title: target,
        icon: FolderOpen,
        used: null,
      });
    }
    return items;
  }, [drives, userFolders, target]);

  return (
    <nav
      className="sidebar"
      aria-label={strings.sidebarLabel}
      data-rail={rail ? 'true' : undefined}
      data-mac={isMac ? 'true' : undefined}
    >
      <DiscardSync />
      <header className="sidebar-head" data-tauri-drag-region>
        {(!rail || narrow) && (
          <span className="brand-mark" data-tauri-drag-region>
            <TreeGlyph size={20} />
            <span className="caption-name" data-tauri-drag-region>{strings.appName}</span>
          </span>
        )}
        {!narrow && (
          <button
            type="button"
            className="icon-btn sidebar-toggle"
            aria-expanded={!collapsed}
            aria-controls="sidebar-body"
            aria-label={collapsed ? strings.sidebarExpand : strings.sidebarCollapse}
            title={collapsed ? strings.sidebarExpand : strings.sidebarCollapse}
            onClick={() => setSidebarCollapsed(!collapsed)}
          >
            <Icon icon={collapsed ? PanelLeftOpen : PanelLeftClose} />
          </button>
        )}
      </header>
      <div id="sidebar-body" className="sidebar-body">
        <ScanViews />
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
              onClick={chooseFolderAndScan}>
              <Icon icon={FolderOpen} className="sidebar-icon" />
              <span className="sidebar-text">
                <span className="sidebar-name">{strings.chooseFolder}</span>
              </span>
            </button>
          )}
        </section>
        {!hasScan && <RecentScans rail={rail} />}
      </div>
      <SettingsPopover />
    </nav>
  );
}
