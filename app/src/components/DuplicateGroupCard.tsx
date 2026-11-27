import type { DuplicateCopy, DuplicateGroup } from '../api';
import { toggleQueued, useDiscardPile } from '../discardPile';
import { allCopiesQueued, type KeepRule } from '../dupesModel';
import { formatBytes, formatFiletime, formatInt } from '../format';
import { strings } from '../strings';
import { FileKindIcon } from './FileKindIcon';
import { CircleAlert, Icon } from './Icon';

export const KEEP_RULES: readonly { rule: KeepRule; label: string }[] = [
  { rule: 'oldest', label: strings.dupesKeepOldest },
  { rule: 'newest', label: strings.dupesKeepNewest },
  { rule: 'shortestPath', label: strings.dupesKeepShortestPath },
];

function AllQueuedWarning({ group }: { group: DuplicateGroup }) {
  const allQueued = useDiscardPile((s) => allCopiesQueued(group, s.idSet));
  if (!allQueued) return null;
  return (
    <span className="dupe-warning" role="status">
      <Icon icon={CircleAlert} size={14} />
      {strings.dupesAllCopiesQueued}
    </span>
  );
}

function DuplicateCopyRow({ copy, writable, onOpen }: {
  copy: DuplicateCopy;
  writable: boolean;
  onOpen: (id: number) => void;
}) {
  const checked = useDiscardPile((s) => s.idSet.has(copy.id));
  const busy = useDiscardPile((s) => s.busy);
  return (
    <tr className={checked ? 'is-queued' : undefined}>
      {writable && (
        <td className="dupe-check-cell">
          <input
            type="checkbox"
            checked={checked}
            disabled={busy}
            aria-label={strings.dupesSelectCopy(copy.path)}
            onChange={() => toggleQueued(copy.id)}
          />
        </td>
      )}
      <td className="dupe-name-cell">
        <button
          type="button"
          className="match-name"
          onClick={() => onOpen(copy.id)}
          title={strings.dupesShowInMap(copy.name)}
        >
          <FileKindIcon node={copy.id} name={copy.name} directory={false} />
          <span className="match-title">{copy.name}</span>
        </button>
      </td>
      <td className="dupe-path-cell" title={copy.path}>{copy.path}</td>
      <td className="dupe-date-cell">{formatFiletime(copy.mtime) || '—'}</td>
    </tr>
  );
}

export function DuplicateGroupCard({ group, writable, busy, onKeep, onOpen }: {
  group: DuplicateGroup;
  writable: boolean;
  busy: boolean;
  onKeep: (rule: KeepRule, groups: readonly DuplicateGroup[]) => void;
  onOpen: (id: number) => void;
}) {
  return (
    <li className="dupe-group">
      <div className="dupe-group-header">
        <div className="dupe-group-stats">
          <strong className="dupe-group-size">{formatBytes(group.size)}</strong>
          <span className="dupe-group-meta">{strings.dupesGroupCopies(formatInt(group.copies.length))}</span>
          <span className="dupe-group-reclaimable">
            {strings.dupesGroupReclaimable(formatBytes(group.reclaimable))}
          </span>
          <code className="dupe-hash" title={strings.dupesHashTitle(group.hash)}>
            {group.hash.slice(0, 12)}
          </code>
          <AllQueuedWarning group={group} />
        </div>
        {writable && (
          <div className="dupe-group-actions" role="group" aria-label={strings.dupesAutoSelectLabel}>
            {KEEP_RULES.map(({ rule, label }) => (
              <button
                key={rule}
                type="button"
                className="dupe-mini-btn"
                disabled={busy}
                onClick={() => onKeep(rule, [group])}
              >
                {label}
              </button>
            ))}
          </div>
        )}
      </div>
      <table className="matches dupe-copies">
        <tbody>
          {group.copies.map((copy) => (
            <DuplicateCopyRow key={copy.id} copy={copy} writable={writable} onOpen={onOpen} />
          ))}
        </tbody>
      </table>
    </li>
  );
}
