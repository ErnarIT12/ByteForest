/** Pure helpers behind the context menu: layout, keyboard movement and the menu contents. */
import type { NodeDetails } from './api';
import { strings } from './strings';

/** Gap kept between the menu and the window edge, CSS pixels. */
export const MENU_MARGIN = 8;

/** Top-left corner for a menu opened at (`x`, `y`), kept fully inside the viewport. */
export function clampMenuPosition(
  x: number,
  y: number,
  width: number,
  height: number,
  viewWidth: number,
  viewHeight: number,
  margin = MENU_MARGIN,
): { left: number; top: number } {
  const fit = (pos: number, size: number, view: number) =>
    Math.max(margin, Math.min(pos, view - size - margin));
  return { left: fit(x, width, viewWidth), top: fit(y, height, viewHeight) };
}

export type MenuKey = 'ArrowDown' | 'ArrowUp' | 'Home' | 'End';

/**
 * Index of the enabled entry to focus after `key`, starting from `current` (-1 for none).
 * Arrow keys wrap around; returns -1 when nothing is enabled.
 */
export function nextEnabledIndex(enabled: boolean[], current: number, key: MenuKey): number {
  const n = enabled.length;
  if (!enabled.some(Boolean)) return -1;
  if (key === 'Home') return enabled.indexOf(true);
  if (key === 'End') return enabled.lastIndexOf(true);
  const step = key === 'ArrowDown' ? 1 : -1;
  let i = current < 0 && step < 0 ? n : current;
  for (let k = 0; k < n; k += 1) {
    i = (i + step + n) % n;
    if (enabled[i]) return i;
  }
  return -1;
}

export type MenuAction =
  | 'open'
  | 'findDupes'
  | 'reveal'
  | 'copy'
  | 'discardAdd'
  | 'discardRemove'
  | 'trash'
  | 'snapshotSave'
  | 'snapshotCompare'
  | 'snapshotStop'
  | 'snapshotRowOpen'
  | 'snapshotRowCompare'
  | 'snapshotRowDelete';

export type MenuEntry =
  | { type: 'separator' }
  | {
      type: 'item';
      action: MenuAction;
      label: string;
      disabled: boolean;
      danger: boolean;
      /** Tooltip; explains why an entry is disabled. */
      title?: string;
    };

/**
 * The entries for one node. The root cannot be trashed or queued, so it gets neither entry.
 * `queued` turns "Add to Discard Pile" into "Remove from Discard Pile".
 */
export function buildMenuEntries(
  details: NodeDetails,
  opts: { isMac: boolean; isRoot: boolean; queued?: boolean; readOnly?: boolean },
): MenuEntry[] {
  const pathBased = !details.has_native_path;
  // A snapshot view never changes the disk.
  const readOnly = opts.readOnly === true;
  const entries: MenuEntry[] = [];
  if (details.node.is_dir) {
    entries.push(
      { type: 'item', action: 'open', label: strings.openFolder, disabled: false, danger: false },
      {
        type: 'item',
        action: 'findDupes',
        label: strings.findDupesHere,
        disabled: readOnly,
        danger: false,
        title: readOnly ? strings.snapshotReadOnly : undefined,
      },
    );
  }
  entries.push(
    {
      type: 'item',
      action: 'reveal',
      label: opts.isMac ? strings.showInFinder : strings.showInExplorer,
      disabled: pathBased,
      danger: false,
    },
    { type: 'item', action: 'copy', label: strings.copyPath, disabled: pathBased, danger: false },
  );
  if (!opts.isRoot) {
    entries.push(
      { type: 'separator' },
      {
        type: 'item',
        action: opts.queued ? 'discardRemove' : 'discardAdd',
        label: opts.queued ? strings.removeFromDiscardPile : strings.addToDiscardPile,
        disabled: opts.queued ? false : pathBased || readOnly,
        danger: false,
        title: readOnly && !opts.queued ? strings.snapshotReadOnly : undefined,
      },
      {
        type: 'item',
        action: 'trash',
        label: `${opts.isMac ? strings.moveToTrash : strings.moveToRecycleBin}…`,
        disabled: pathBased || readOnly,
        danger: true,
        title: readOnly ? strings.snapshotReadOnly : undefined,
      },
    );
  }
  return entries;
}

/**
 * The entries of the menu of one saved snapshot in the sidebar. `compareReason` is why
 * "Compare with Current" is unavailable (null when it is available).
 */
export function buildSnapshotRowEntries(opts: { compareReason: string | null }): MenuEntry[] {
  return [
    { type: 'item', action: 'snapshotRowOpen', label: strings.snapshotOpen, disabled: false, danger: false },
    {
      type: 'item',
      action: 'snapshotRowCompare',
      label: strings.snapshotCompareCurrent,
      disabled: opts.compareReason !== null,
      danger: false,
      title: opts.compareReason ?? undefined,
    },
    { type: 'separator' },
    { type: 'item', action: 'snapshotRowDelete', label: strings.snapshotDelete, disabled: false, danger: true },
  ];
}

/** The entries of the Snapshots menu; "Stop comparing" is only offered while comparing. */
export function buildSnapshotEntries(opts: { comparing: boolean }): MenuEntry[] {
  const item = (action: MenuAction, label: string): MenuEntry => ({
    type: 'item',
    action,
    label,
    disabled: false,
    danger: false,
  });
  const entries = [
    item('snapshotSave', strings.snapshotSave),
    item('snapshotCompare', strings.snapshotCompare),
  ];
  if (opts.comparing) entries.push({ type: 'separator' }, item('snapshotStop', strings.snapshotStop));
  return entries;
}

/**
 * What a Delete key press on a focused item means: `queue` adds it to the Discard Pile (no
 * deletion), Shift+Delete asks to move it to the Recycle Bin / Trash right away.
 */
export function deleteKeyIntent(e: {
  key: string;
  shiftKey: boolean;
  ctrlKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
}): 'queue' | 'trash' | null {
  if (e.key !== 'Delete' || e.ctrlKey || e.altKey || e.metaKey) return null;
  return e.shiftKey ? 'trash' : 'queue';
}

/** Whether the key event asks for a context menu from the keyboard (Shift+F10 or Menu key). */
export function isContextMenuKey(e: { key: string; shiftKey: boolean; ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean }): boolean {
  if (e.ctrlKey || e.altKey || e.metaKey) return false;
  return e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey);
}
