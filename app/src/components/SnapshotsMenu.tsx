import { useRef, useState } from 'react';
import { buildSnapshotEntries } from '../contextMenu';
import { runSnapshotAction } from '../snapshots';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { ContextMenu } from './ContextMenu';

/** Top bar button that opens the Snapshots menu (save, compare, stop comparing). */
export function SnapshotsMenu() {
  const button = useRef<HTMLButtonElement>(null);
  const closedAt = useRef(0);
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const [busy, setBusy] = useState(false);
  const scanning = useAppStore((s) => s.scanning);
  const hasResult = useAppStore((s) => s.resultScanId !== null);
  const comparing = useAppStore((s) => s.baseline !== null);

  const open = () => {
    // The menu closes itself on pointer down outside it, which would reopen it right away.
    if (Date.now() - closedAt.current < 200) return;
    const box = button.current?.getBoundingClientRect();
    if (box) setAt({ x: box.left, y: box.bottom + 4 });
  };
  const close = () => {
    closedAt.current = Date.now();
    setAt(null);
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        className="btn"
        aria-haspopup="menu"
        aria-expanded={at !== null}
        disabled={scanning || !hasResult || busy}
        onClick={open}
      >
        {strings.snapshots}
      </button>
      {at && (
        <ContextMenu
          x={at.x}
          y={at.y}
          label={strings.snapshotsMenuLabel}
          entries={buildSnapshotEntries({ comparing })}
          onSelect={(action) => {
            close();
            setBusy(true);
            void runSnapshotAction(action).finally(() => setBusy(false));
          }}
          onClose={close}
        />
      )}
    </>
  );
}
