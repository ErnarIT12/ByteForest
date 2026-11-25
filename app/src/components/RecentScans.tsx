import { useEffect, useState } from 'react';
import type { AutoSnapshot } from '../api';
import { buildSnapshotRowEntries } from '../contextMenu';
import { formatBytes, formatInt } from '../format';
import {
  compareWithAutoSnapshotId,
  openAutoSnapshot,
  removeAutoSnapshot,
} from '../snapshots';
import {
  canOpenSnapshot,
  compareDisabledReason,
  countSnapshots,
  formatRelativeTime,
  formatUnixMsFull,
  limitSnapshotRows,
  groupSnapshots,
} from '../snapshotsModel';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { ContextMenu } from './ContextMenu';
import { ConfirmDialog } from './FileActionsHost';
import { useAutoSnapshots } from '../autoSnapshots';
import { runScan } from '../scanController';
import { locationOf } from '../scanTargets';
import { Clock, Ellipsis, History, Icon } from './Icon';

/** Snapshot rows shown before "Show all". */
export const VISIBLE_SNAPSHOT_ROWS = 10;

interface Props {
  /** Icon rail: only the recent scans, no snapshot rows. */
  rail: boolean;
  /** Scans `target` again. */
  detailed?: boolean;
}

/** Sidebar section: recent scans (one click rescans) with their saved snapshots below. */
export function RecentScans({ rail, detailed }: Props) {
  const recent = useAppStore((s) => s.recentScans);
  const snapshots = useAutoSnapshots((s) => s.list);
  const groups = groupSnapshots(recent, snapshots);
  const onScan = (target: string) => {
    useAppStore.getState().setTarget(locationOf(target));
    void runScan(target);
  };
  const [showAll, setShowAll] = useState(false);
  // Relative dates ('2 h ago') are refreshed once a minute.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(timer);
  }, []);
  const [menu, setMenu] = useState<{ snap: AutoSnapshot; x: number; y: number } | null>(null);
  const [confirm, setConfirm] = useState<{ snap: AutoSnapshot; busy: boolean } | null>(null);
  const scanning = useAppStore((s) => s.scanning);
  const resultScanId = useAppStore((s) => s.resultScanId);
  const resultTarget = useAppStore((s) => s.resultTarget);
  const resultSource = useAppStore((s) => s.resultSource);

  const total = countSnapshots(groups);
  const shown = rail
    ? groups.filter((g) => g.recent)
    : showAll
      ? groups
      : limitSnapshotRows(groups, VISIBLE_SNAPSHOT_ROWS);
  const canOpen = canOpenSnapshot({ scanning });

  const compareReason = (snap: AutoSnapshot) =>
    compareDisabledReason({ scanning, resultScanId, resultTarget, resultSource }, snap.label);

  return (
    <section className="sidebar-section" aria-labelledby="sidebar-recent">
      <h2 id="sidebar-recent" className="sidebar-heading">{detailed ? 'Saved scans by location' : strings.recentTitle}</h2>
      {shown.length === 0 ? (
        <p className="sidebar-note">{strings.recentEmpty}</p>
      ) : (
        <ul className="sidebar-list">
          {shown.map((group) => (
            <li key={group.target.toLowerCase()}>
              {group.recent ? (
                <button type="button" className="sidebar-item" title={group.target}
                  aria-label={strings.recentRescan(group.label)} onClick={() => onScan(group.target)}>
                  <Icon icon={Clock} className="sidebar-icon" />
                  <span className="sidebar-text">
                    <span className="sidebar-name">{group.label}</span>
                    <span className="sidebar-detail">
                      {formatBytes(group.recent.bytes)} · {formatRelativeTime(group.recent.finishedAt, now)}
                    </span>
                  </span>
                </button>
              ) : (
                <div className="sidebar-item snapshot-head" title={group.target}>
                  <Icon icon={History} className="sidebar-icon" />
                  <span className="sidebar-text">
                    <span className="sidebar-name">{group.label}</span>
                  </span>
                </div>
              )}
              {!rail && group.snapshots.length > 0 && (
                <ul className="snapshot-list" aria-label={`${strings.snapshotRowsLabel}: ${group.label}`}>
                  {group.snapshots.map((snap) => {
                    const when = formatUnixMsFull(snap.created_unix_ms);
                    return (
                      <li key={snap.id} className="snapshot-row">
                        <button type="button" className="snapshot-open" disabled={!canOpen}
                          title={`${when}\n${group.target}\n${formatInt(snap.node_count)} items`}
                          aria-label={strings.snapshotOpenAria(group.label, when)}
                          onClick={() => void openAutoSnapshot(snap.id)}>
                          <span className="snapshot-when">{detailed ? when : formatRelativeTime(snap.created_unix_ms, now)}</span>
                          <span className="snapshot-size">{formatBytes(snap.bytes_on_disk)}</span>
                        </button>
                        {detailed && <button type="button" className="btn" disabled={compareReason(snap) !== null}
                          title={compareReason(snap) ?? 'Compare with the current scan'}
                          onClick={() => void compareWithAutoSnapshotId(snap.id)}>Compare</button>}
                        <button type="button" className="icon-btn snapshot-more" aria-haspopup="menu"
                          aria-label={strings.snapshotMoreActions} title={strings.snapshotMoreActions}
                          onClick={(e) => {
                            const box = e.currentTarget.getBoundingClientRect();
                            setMenu({ snap, x: box.left, y: box.bottom + 4 });
                          }}>
                          <Icon icon={Ellipsis} size={14} />
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </li>
          ))}
        </ul>
      )}
      {!rail && total > VISIBLE_SNAPSHOT_ROWS && (
        <button type="button" className="sidebar-item sidebar-action" onClick={() => setShowAll(!showAll)}>
          <span className="sidebar-text">
            <span className="sidebar-name">
              {showAll ? strings.snapshotShowFewer : strings.snapshotShowAll(formatInt(total))}
            </span>
          </span>
        </button>
      )}
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          label={strings.snapshotMoreActions}
          entries={buildSnapshotRowEntries({ compareReason: compareReason(menu.snap) })}
          onSelect={(action) => {
            const { snap } = menu;
            setMenu(null);
            if (action === 'snapshotRowOpen') void openAutoSnapshot(snap.id);
            else if (action === 'snapshotRowCompare') void compareWithAutoSnapshotId(snap.id);
            else if (action === 'snapshotRowDelete') setConfirm({ snap, busy: false });
          }}
          onClose={() => setMenu(null)}
        />
      )}
      {confirm && (
        <ConfirmDialog
          idBase="snapshot-delete"
          title={strings.snapshotDeleteTitle}
          busy={confirm.busy}
          confirmLabel={strings.snapshotDeleteConfirm}
          onCancel={() => !confirm.busy && setConfirm(null)}
          onConfirm={() => {
            setConfirm({ ...confirm, busy: true });
            void removeAutoSnapshot(confirm.snap.id).finally(() => setConfirm(null));
          }}
        >
          <p>
            {strings.snapshotDeleteBody(
              confirm.snap.label,
              formatUnixMsFull(confirm.snap.created_unix_ms),
            )}
          </p>
        </ConfirmDialog>
      )}
    </section>
  );
}
