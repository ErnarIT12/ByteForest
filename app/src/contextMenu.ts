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

export type MenuAction = 'open' | 'reveal' | 'copy' | 'trash';

export type MenuEntry =
  | { type: 'separator' }
  | { type: 'item'; action: MenuAction; label: string; disabled: boolean; danger: boolean };

/** The entries for one node. The root cannot be trashed, so it gets no trash entry. */
export function buildMenuEntries(
  details: NodeDetails,
  opts: { isMac: boolean; isRoot: boolean },
): MenuEntry[] {
  const pathBased = !details.has_native_path;
  const entries: MenuEntry[] = [];
  if (details.node.is_dir) {
    entries.push({ type: 'item', action: 'open', label: strings.openFolder, disabled: false, danger: false });
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
        action: 'trash',
        label: `${opts.isMac ? strings.moveToTrash : strings.moveToRecycleBin}…`,
        disabled: pathBased,
        danger: true,
      },
    );
  }
  return entries;
}

/** Whether the key event asks for a context menu from the keyboard (Shift+F10 or Menu key). */
export function isContextMenuKey(e: { key: string; shiftKey: boolean; ctrlKey?: boolean; altKey?: boolean; metaKey?: boolean }): boolean {
  if (e.ctrlKey || e.altKey || e.metaKey) return false;
  return e.key === 'ContextMenu' || (e.key === 'F10' && e.shiftKey);
}
