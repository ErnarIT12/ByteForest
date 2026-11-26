import { beforeEach, describe, expect, it } from 'vitest';
import { SCAN_CANCELLED, type NodeView, type OpenedSnapshot, type ScanDone } from './api';
import { acceptEvent, useAppStore } from './store';

const node = (id: number, name: string): NodeView => ({
  id,
  name,
  size: 100,
  alloc: 100,
  files: 1,
  dirs: 0,
  is_dir: true,
  has_children: true,
  mtime: 0,
  percent_of_parent: 0,
});

const done = (scanId: number): ScanDone => ({
  scan_id: scanId,
  root: node(0, 'C:'),
  stats: {
    files: 1,
    dirs: 1,
    bytes: 100,
    elapsed_ms: 10,
    method: 'Mft',
    fallback_reason: null,
    skipped_records: 0,
    errors: 0,
  },
});

/** Runs a full scan (start, id resolved, done) so the store has a root. */
function scanned() {
  const s = useAppStore.getState();
  s.beginScan('C');
  s.scanResolved(1);
  useAppStore.getState().handleDone(done(1));
}

describe('store navigation', () => {
  beforeEach(() => useAppStore.getState().reset());

  it('goUp at the root changes nothing', () => {
    scanned();
    useAppStore.getState().select(5);
    const before = useAppStore.getState();
    before.goUp();
    const after = useAppStore.getState();
    expect(after.focusId).toBe(0);
    expect(after.selectedId).toBe(5);
    expect(after.focusPath).toEqual(before.focusPath);
  });

  it('setFocus sets focusId and clears the selection', () => {
    scanned();
    useAppStore.getState().select(7);
    useAppStore.getState().setFocus(3, [
      { id: 0, name: 'C:' },
      { id: 3, name: 'Users' },
    ]);
    const s = useAppStore.getState();
    expect(s.focusId).toBe(3);
    expect(s.selectedId).toBeNull();
    expect(s.focusPath.map((c) => c.id)).toEqual([0, 3]);
  });

  it('goUp returns to the parent and selects the folder it left', () => {
    scanned();
    useAppStore.getState().setFocus(3, [
      { id: 0, name: 'C:' },
      { id: 3, name: 'Users' },
    ]);
    useAppStore.getState().goUp();
    const s = useAppStore.getState();
    expect(s.focusId).toBe(0);
    expect(s.selectedId).toBe(3);
  });

  it('a done event that arrives before startScan resolves is kept', () => {
    const s = useAppStore.getState();
    s.beginScan('C');
    useAppStore.getState().handleDone(done(1));
    useAppStore.getState().scanResolved(1);
    const after = useAppStore.getState();
    expect(after.scanning).toBe(false);
    expect(after.rootId).toBe(0);
  });

  it('ignores the superseded scan while the new one is pending', () => {
    scanned();
    useAppStore.getState().beginScan('C');
    // Starting scan 2 cancels scan 1, which reports "cancelled" before startScan resolves.
    useAppStore.getState().handleError({ scan_id: 1, message: 'cancelled' });
    expect(useAppStore.getState().scanning).toBe(true);
    useAppStore.getState().handleProgress({ scan_id: 2, files: 5, dirs: 1, bytes: 9, phase: 'Parsing' });
    expect(useAppStore.getState().progress?.files).toBe(5);
    useAppStore.getState().scanResolved(2);
    useAppStore.getState().handleDone(done(2));
    expect(useAppStore.getState().scanning).toBe(false);
    expect(useAppStore.getState().generation).toBe(2);
  });

  it('a cancelled scan returns to idle without an error', () => {
    useAppStore.getState().beginScan('C');
    useAppStore.getState().scanResolved(1);
    useAppStore.getState().handleError({ scan_id: 1, message: SCAN_CANCELLED });
    const s = useAppStore.getState();
    expect(s.scanning).toBe(false);
    expect(s.error).toBeNull();
    expect(s.lastOutcome).toBe('cancelled');
  });
});

describe('acceptEvent', () => {
  it('accepts a newer id while startScan is pending', () => {
    expect(acceptEvent({ pending: true, current: 1, lastKnown: 1 }, 2)).toBe(true);
  });

  it('accepts only the exact id once resolved', () => {
    const gate = { pending: false, current: 3, lastKnown: 3 };
    expect(acceptEvent(gate, 3)).toBe(true);
    expect(acceptEvent(gate, 4)).toBe(false);
  });

  it('rejects an older id', () => {
    expect(acceptEvent({ pending: true, current: 2, lastKnown: 2 }, 1)).toBe(false);
    expect(acceptEvent({ pending: false, current: 2, lastKnown: 2 }, 1)).toBe(false);
    expect(acceptEvent({ pending: false, current: null, lastKnown: 0 }, 1)).toBe(false);
  });
});

describe('scan target', () => {
  beforeEach(() => useAppStore.getState().reset());

  it('keeps the exact scanned target with the result', () => {
    const s = useAppStore.getState();
    s.beginScan('C:\\Users\\me');
    s.scanResolved(1);
    useAppStore.getState().handleDone(done(1));
    expect(useAppStore.getState().resultTarget).toBe('C:\\Users\\me');
  });

  it('clears a stale error title when a scan error replaces it', () => {
    const s = useAppStore.getState();
    s.elevationFailed('spawn failed');
    expect(useAppStore.getState().errorTitle).not.toBeNull();
    s.beginScan('C:');
    s.scanResolved(1);
    useAppStore.getState().handleError({ scan_id: 1, message: 'boom' });
    expect(useAppStore.getState().errorTitle).toBeNull();
    expect(useAppStore.getState().error).toBe('boom');
  });
});

describe('store applyRemoval', () => {
  beforeEach(() => useAppStore.getState().reset());

  it('subtracts the totals, selects the parent and asks views to refresh', () => {
    scanned();
    const before = useAppStore.getState();
    before.applyRemoval(5, { bytes: 40, files: 1, dirs: 0, parent: 3 });
    const s = useAppStore.getState();
    expect(s.root).toMatchObject({ size: 60, files: 0, dirs: 0 });
    expect(s.stats).toMatchObject({ bytes: 60, files: 0 });
    expect(s.selectedId).toBe(3);
    expect(s.generation).toBe(before.generation + 1);
    expect(s.invalidation).toEqual({ seq: 1, nodeId: 5, parentId: 3 });
  });

  it('never lets a total go below zero', () => {
    scanned();
    useAppStore.getState().applyRemoval(5, { bytes: 1e9, files: 9, dirs: 9, parent: 0 });
    expect(useAppStore.getState().root).toMatchObject({ size: 0, files: 0, dirs: 0 });
  });

  it('moves the focus to the parent when the focused folder is removed', () => {
    scanned();
    useAppStore.getState().setFocus(3, [
      { id: 0, name: 'C:' },
      { id: 3, name: 'Users' },
    ]);
    useAppStore.getState().applyRemoval(3, { bytes: 10, files: 1, dirs: 1, parent: 0 });
    const s = useAppStore.getState();
    expect(s.focusId).toBe(0);
    expect(s.focusPath.map((c) => c.id)).toEqual([0]);
    expect(s.reveal?.path).toEqual([0]);
  });

  it('keeps the focus when something inside it is removed', () => {
    scanned();
    useAppStore.getState().applyRemoval(5, { bytes: 1, files: 1, dirs: 0, parent: 0 });
    expect(useAppStore.getState().focusId).toBe(0);
  });
});

describe('store applyBatchRemoval', () => {
  beforeEach(() => useAppStore.getState().reset());
  const batch = (moved: { id: number; parent: number; bytes: number }[]) => ({
    moved: moved.map((m) => ({ ...m, files: 1, dirs: 0 })),
    failed: [],
    bytes: moved.reduce((n, m) => n + m.bytes, 0),
    files: moved.length,
    dirs: 0,
  });

  it('applies every removal at once: totals, one generation bump, one invalidation', () => {
    scanned();
    const before = useAppStore.getState();
    before.applyBatchRemoval(batch([
      { id: 5, parent: 0, bytes: 30 },
      { id: 6, parent: 3, bytes: 20 },
    ]));
    const s = useAppStore.getState();
    expect(s.root).toMatchObject({ size: 50, files: 0 });
    expect(s.stats).toMatchObject({ bytes: 50 });
    expect(s.generation).toBe(before.generation + 1);
    expect(s.invalidation).toMatchObject({ seq: 1, all: true });
  });

  it('selects the parent of a removed selection', () => {
    scanned();
    useAppStore.getState().select(6);
    useAppStore.getState().applyBatchRemoval(batch([{ id: 6, parent: 3, bytes: 1 }]));
    expect(useAppStore.getState().selectedId).toBe(3);
  });

  it('selects the surviving ancestor of a selection inside a removed folder', () => {
    scanned();
    useAppStore.getState().select(9);
    useAppStore.getState().applyBatchRemoval(batch([{ id: 6, parent: 3, bytes: 1 }]), [0, 3, 6, 9]);
    expect(useAppStore.getState().selectedId).toBe(3);
  });

  it('keeps an unaffected selection and moves the focus out of a removed folder', () => {
    scanned();
    useAppStore.getState().setFocus(3, [
      { id: 0, name: 'C:' },
      { id: 3, name: 'Users' },
    ]);
    useAppStore.getState().select(7);
    useAppStore.getState().applyBatchRemoval(batch([{ id: 3, parent: 0, bytes: 1 }]), [0, 7]);
    const s = useAppStore.getState();
    expect(s.selectedId).toBe(7);
    expect(s.focusId).toBe(0);
  });

  it('ignores an empty batch', () => {
    scanned();
    const before = useAppStore.getState().generation;
    useAppStore.getState().applyBatchRemoval(batch([]));
    expect(useAppStore.getState().generation).toBe(before);
  });
});

describe('search preset', () => {
  beforeEach(() => useAppStore.getState().reset());

  it('numbers every request so an identical preset is applied again', () => {
    const preset = { category: 'video' as const, under: { id: 3, name: 'Movies' } };
    useAppStore.getState().openSearch(preset);
    useAppStore.getState().openSearch(preset);
    expect(useAppStore.getState().resultView).toBe('files');
    expect(useAppStore.getState().searchPreset).toEqual({ ...preset, seq: 2 });
  });
});

describe('snapshot comparison', () => {
  const info = {
    label: 'C:',
    created_unix_ms: 1_700_000_000_000,
    total_delta: 500,
    added: { bytes: 600, count: 3 },
    removed: { bytes: 100, count: 1 },
  };
  beforeEach(() => useAppStore.getState().reset());

  it('setBaseline stores the comparison and asks views to refetch', () => {
    scanned();
    const before = useAppStore.getState();
    before.setBaseline(info);
    const s = useAppStore.getState();
    expect(s.baseline).toEqual(info);
    expect(s.generation).toBe(before.generation + 1);
    expect(s.baselineSeq).toBe(before.baselineSeq + 1);
    s.setBaseline(null);
    expect(useAppStore.getState().baseline).toBeNull();
    expect(useAppStore.getState().generation).toBe(before.generation + 2);
  });

  it('toggling the comparison keeps the focus and selection', () => {
    scanned();
    const st = useAppStore.getState();
    st.setFocus(5);
    st.select(7);
    const before = useAppStore.getState();
    before.setBaseline(info);
    let s = useAppStore.getState();
    expect([s.focusId, s.selectedId]).toEqual([before.focusId, before.selectedId]);
    s.setBaseline(null);
    s = useAppStore.getState();
    expect([s.focusId, s.selectedId]).toEqual([before.focusId, before.selectedId]);
  });

  it('a removal lowers the net change by the removed bytes', () => {
    scanned();
    useAppStore.getState().setBaseline(info);
    useAppStore.getState().applyRemoval(5, { bytes: 40, files: 1, dirs: 0, parent: 0 });
    expect(useAppStore.getState().baseline?.total_delta).toBe(460);
  });

  it('a new scan stops the comparison', () => {
    scanned();
    useAppStore.getState().setBaseline(info);
    useAppStore.getState().beginScan('C');
    expect(useAppStore.getState().baseline).toBeNull();
  });

  it('a new result never keeps the previous comparison', () => {
    scanned();
    useAppStore.getState().setBaseline(info);
    useAppStore.getState().handleDone(done(1));
    expect(useAppStore.getState().baseline).toBeNull();
  });
});

describe('excluded summary', () => {
  beforeEach(() => useAppStore.getState().reset());

  const finish = (excluded?: ScanDone['excluded']) => {
    const s = useAppStore.getState();
    s.beginScan('C');
    s.scanResolved(1);
    useAppStore.getState().handleDone({ ...done(1), excluded });
  };

  it('is kept from the done event when something was excluded', () => {
    finish({ items: 3, bytes: 4096 });
    expect(useAppStore.getState().excluded).toEqual({ items: 3, bytes: 4096 });
  });

  it('is null when nothing was excluded', () => {
    finish({ items: 0, bytes: 0 });
    expect(useAppStore.getState().excluded).toBeNull();
    useAppStore.getState().reset();
    finish();
    expect(useAppStore.getState().excluded).toBeNull();
  });
});

describe('recent scans', () => {
  beforeEach(() => useAppStore.getState().reset());

  it('records a finished scan once, newest first', () => {
    const s = useAppStore.getState();
    s.beginScan('C:');
    s.scanResolved(1);
    s.handleDone(done(1));
    s.beginScan('D:');
    s.scanResolved(2);
    s.handleDone(done(2));
    s.beginScan('C:');
    s.scanResolved(3);
    s.handleDone(done(3));
    expect(useAppStore.getState().recentScans.map((r) => r.target)).toEqual(['C:', 'D:']);
  });

  it('ignores a scan that failed', () => {
    const s = useAppStore.getState();
    s.beginScan('C:');
    s.scanResolved(1);
    s.handleError({ scan_id: 1, message: 'boom' });
    expect(useAppStore.getState().recentScans).toEqual([]);
  });
});

describe('opened snapshots', () => {
  beforeEach(() => useAppStore.getState().reset());

  const opened = (scanId: number): OpenedSnapshot => ({
    scan_id: scanId,
    target: 'C:',
    files: 5,
    dirs: 2,
    bytes: 100,
    root: node(0, 'C:'),
    source: { kind: 'snapshot', created_unix_ms: 1234 },
  });

  it('shows the snapshot as a read-only result and leaves the recent scans alone', () => {
    scanned();
    const before = useAppStore.getState().recentScans;
    useAppStore.getState().setBaseline({
      label: 'C:', created_unix_ms: 1, total_delta: 5,
      added: { bytes: 0, count: 0 }, removed: { bytes: 0, count: 0 },
    });
    useAppStore.getState().handleOpened(opened(7));
    const s = useAppStore.getState();
    expect(s.resultScanId).toBe(7);
    expect(s.resultSource).toEqual({ kind: 'snapshot', created_unix_ms: 1234 });
    expect(s.resultTarget).toBe('C:');
    expect(s.target).toBe('C');
    expect(s.baseline).toBeNull();
    expect(s.scanning).toBe(false);
    expect(s.stats?.files).toBe(5);
    expect(s.focusPath).toEqual([{ id: 0, name: 'C:' }]);
    expect(s.recentScans).toBe(before);
    expect(acceptEvent(s.gate, 7)).toBe(true);
  });

  it('becomes live again after a scan finishes', () => {
    useAppStore.getState().handleOpened(opened(7));
    const s = useAppStore.getState();
    s.beginScan('C:');
    s.scanResolved(8);
    useAppStore.getState().handleDone(done(8));
    expect(useAppStore.getState().resultSource).toEqual({ kind: 'live' });
  });
});
