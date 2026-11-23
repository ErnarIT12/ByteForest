import { describe, expect, it } from 'vitest';
import type { NodeDetails } from './api';
import {
  buildMenuEntries,
  deleteKeyIntent,
  buildSnapshotEntries,
  clampMenuPosition,
  isContextMenuKey,
  nextEnabledIndex,
} from './contextMenu';

const details = (isDir: boolean, native = true): NodeDetails => ({
  node: {
    id: 3, name: 'x', size: 1, alloc: 1, files: 1, dirs: 0, is_dir: isDir,
    has_children: isDir, mtime: 0, percent_of_parent: 0,
  },
  path: '/files/x',
  has_native_path: native,
});

describe('clampMenuPosition', () => {
  it('keeps a menu that fits where it was opened', () => {
    expect(clampMenuPosition(100, 120, 200, 150, 800, 600)).toEqual({ left: 100, top: 120 });
  });
  it('pulls a menu back from the right and bottom edges', () => {
    expect(clampMenuPosition(790, 590, 200, 150, 800, 600)).toEqual({ left: 592, top: 442 });
  });
  it('never goes past the top-left margin, even in a tiny window', () => {
    expect(clampMenuPosition(-5, -5, 500, 500, 100, 100)).toEqual({ left: 8, top: 8 });
  });
});

describe('nextEnabledIndex', () => {
  const enabled = [true, false, true, true];
  it('skips disabled entries and wraps', () => {
    expect(nextEnabledIndex(enabled, 0, 'ArrowDown')).toBe(2);
    expect(nextEnabledIndex(enabled, 3, 'ArrowDown')).toBe(0);
    expect(nextEnabledIndex(enabled, 0, 'ArrowUp')).toBe(3);
    expect(nextEnabledIndex(enabled, -1, 'ArrowDown')).toBe(0);
    expect(nextEnabledIndex(enabled, -1, 'ArrowUp')).toBe(3);
  });
  it('Home and End pick the first and last enabled entry', () => {
    expect(nextEnabledIndex([false, true, true, false], 2, 'Home')).toBe(1);
    expect(nextEnabledIndex([false, true, true, false], 1, 'End')).toBe(2);
  });
  it('returns -1 when nothing is enabled', () => {
    expect(nextEnabledIndex([false, false], 0, 'ArrowDown')).toBe(-1);
  });
});

describe('buildMenuEntries', () => {
  const actions = (entries: ReturnType<typeof buildMenuEntries>) =>
    entries.map((e) => (e.type === 'item' ? e.action : '-'));

  it('offers open only for folders and trash last as a danger item', () => {
    expect(actions(buildMenuEntries(details(true), { isMac: false, isRoot: false }))).toEqual([
      'open', 'reveal', 'copy', '-', 'discardAdd', 'trash',
    ]);
    const file = buildMenuEntries(details(false), { isMac: false, isRoot: false });
    expect(actions(file)).toEqual(['reveal', 'copy', '-', 'discardAdd', 'trash']);
    expect(file.at(-1)).toMatchObject({ danger: true, label: 'Move to Recycle Bin…' });
  });
  it('uses platform wording', () => {
    const mac = buildMenuEntries(details(false), { isMac: true, isRoot: false });
    expect(mac.map((e) => (e.type === 'item' ? e.label : ''))).toContain('Show in Finder');
    expect(mac.at(-1)).toMatchObject({ label: 'Move to Trash…' });
  });
  it('toggles the Discard Pile entry for queued nodes', () => {
    const entries = buildMenuEntries(details(false), { isMac: false, isRoot: false, queued: true });
    expect(actions(entries)).toEqual(['reveal', 'copy', '-', 'discardRemove', 'trash']);
    expect(entries.find((e) => e.type === 'item' && e.action === 'discardRemove')).toMatchObject({
      label: 'Remove from Discard Pile',
      disabled: false,
      danger: false,
    });
  });
  it('never offers to trash or queue the root', () => {
    expect(actions(buildMenuEntries(details(true), { isMac: false, isRoot: true }))).toEqual([
      'open', 'reveal', 'copy',
    ]);
  });
  it('disables path based entries without a native path but keeps open', () => {
    const entries = buildMenuEntries(details(true, false), { isMac: false, isRoot: false });
    const disabled = entries.flatMap((e) => (e.type === 'item' && e.disabled ? [e.action] : []));
    expect(disabled).toEqual(['reveal', 'copy', 'discardAdd', 'trash']);
  });
});

describe('deleteKeyIntent', () => {
  it('queues on Delete and asks to trash on Shift+Delete', () => {
    expect(deleteKeyIntent({ key: 'Delete', shiftKey: false })).toBe('queue');
    expect(deleteKeyIntent({ key: 'Delete', shiftKey: true })).toBe('trash');
  });
  it('ignores other keys and modified Delete', () => {
    expect(deleteKeyIntent({ key: 'Backspace', shiftKey: false })).toBeNull();
    expect(deleteKeyIntent({ key: 'Delete', shiftKey: false, ctrlKey: true })).toBeNull();
    expect(deleteKeyIntent({ key: 'Delete', shiftKey: false, metaKey: true })).toBeNull();
  });
});

describe('isContextMenuKey', () => {
  it('accepts the Menu key and Shift+F10 only', () => {
    expect(isContextMenuKey({ key: 'ContextMenu', shiftKey: false })).toBe(true);
    expect(isContextMenuKey({ key: 'F10', shiftKey: true })).toBe(true);
    expect(isContextMenuKey({ key: 'F10', shiftKey: false })).toBe(false);
    expect(isContextMenuKey({ key: 'F10', shiftKey: true, ctrlKey: true })).toBe(false);
  });
});

describe('buildSnapshotEntries', () => {
  const actions = (comparing: boolean) =>
    buildSnapshotEntries({ comparing }).flatMap((e) => (e.type === 'item' ? [e.action] : []));

  it('offers save and compare', () => {
    expect(actions(false)).toEqual(['snapshotSave', 'snapshotCompare']);
  });

  it('adds stop comparing only while comparing', () => {
    expect(actions(true)).toEqual(['snapshotSave', 'snapshotCompare', 'snapshotStop']);
  });

  it('never disables an entry', () => {
    expect(buildSnapshotEntries({ comparing: true }).every((e) => e.type !== 'item' || !e.disabled)).toBe(true);
  });
});
