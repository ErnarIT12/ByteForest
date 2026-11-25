import { FileKindIcon } from './FileKindIcon';
import { memo } from 'react';
import { deltaCell } from '../diffView';
import { formatBytes, formatCount, formatFiletime, formatPercent } from '../format';
import { strings } from '../strings';
import type { NodeView } from '../api';
import { ChevronRight, Icon, Trash2 } from './Icon';

const INDENT_PX = 16;

interface Props {
  node: NodeView;
  /** Passed separately: the model mutates entries in place, so memo needs plain values. */
  expanded: boolean;
  isRoot: boolean;
  level: number;
  /** Show the change column (comparing with a snapshot). */
  showDelta: boolean;
  selected: boolean;
  focused: boolean;
  /** In the Discard Pile. */
  queued: boolean;
  /** Offset in the virtual list, pixels. */
  top: number;
  onSelect(id: number): void;
  onToggle(id: number): void;
  onActivate(id: number): void;
  /** Right click (pointer position) or the Menu key (position of the row). */
  onContextMenu(id: number, x: number, y: number): void;
}

function Chevron({ open }: { open: boolean }) {
  return <Icon icon={ChevronRight} size={14} className={`chevron${open ? ' open' : ''}`} />;
}

/** One line of the folder tree: name with indent and chevron, size, share, files, date. */
export const TreeRow = memo(function TreeRow({
  node,
  expanded,
  isRoot,
  level,
  showDelta,
  selected,
  focused,
  queued,
  top,
  onSelect,
  onToggle,
  onActivate,
  onContextMenu,
}: Props) {
  const expandable = node.is_dir && node.has_children;
  const percent = Math.min(100, Math.max(0, node.percent_of_parent));
  const change = showDelta ? deltaCell(node.diff) : null;

  return (
    <div
      id={`tree-row-${node.id}`}
      role="treeitem"
      aria-level={level}
      aria-selected={selected}
      aria-expanded={expandable ? expanded : undefined}
      className={`tree-row${selected ? ' selected' : ''}${focused ? ' focused' : ''}`}
      style={{ transform: `translateY(${top}px)` }}
      onClick={() => onSelect(node.id)}
      onDoubleClick={() => node.is_dir && onActivate(node.id)}
      onContextMenu={(e) => {
        e.preventDefault();
        onSelect(node.id);
        onContextMenu(node.id, e.clientX, e.clientY);
      }}
    >
      <div className="cell name" style={{ paddingLeft: (level - 1) * INDENT_PX + 4 }}>
        {expandable ? (
          <button
            type="button"
            className="disclosure"
            tabIndex={-1}
            aria-label={expanded ? strings.collapse : strings.expand}
            onClick={(e) => {
              e.stopPropagation();
              onToggle(node.id);
            }}
          >
            <Chevron open={expanded} />
          </button>
        ) : (
          <span className="disclosure-spacer" />
        )}
        <FileKindIcon name={node.name} directory={node.is_dir} />
        <span className="name-text" title={node.name}>
          {node.name}
        </span>
        {queued && <Icon icon={Trash2} size={13} className="queued-mark" label={strings.inDiscardPile} />}
      </div>
      <div className="cell num">{formatBytes(node.size)}</div>
      {showDelta && (
        <div className="cell num col-delta">
          {change && <span className={`delta ${change.tone}`}>{change.text}</span>}
        </div>
      )}
      <div className="cell percent">
        {!isRoot && (
          <>
            <span className="bar" aria-hidden="true">
              <span className="bar-fill" style={{ width: `${percent}%` }} />
            </span>
            <span className="percent-text">{formatPercent(percent)}</span>
          </>
        )}
      </div>
      <div className="cell num col-files">{node.is_dir ? formatCount(node.files) : ''}</div>
      <div className="cell date col-modified">{formatFiletime(node.mtime)}</div>
    </div>
  );
});

/** Placeholder line under a directory whose next page is loading. */
export function MoreRow({ level, failed, top, onLoad }: {
  level: number;
  failed: boolean;
  top: number;
  onLoad?: () => void;
}) {
  return (
    <div
      className="tree-row more"
      style={{ transform: `translateY(${top}px)` }}
      aria-hidden={onLoad ? undefined : true}
    >
      <div className="cell name" style={{ paddingLeft: (level - 1) * INDENT_PX + 26 }}>
        {failed ? strings.loadFailed : onLoad ? (
          <button className="tree-gap" type="button" onClick={onLoad}>Load more entries</button>
        ) : strings.loadingMore}
      </div>
    </div>
  );
}
