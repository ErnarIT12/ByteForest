import { useEffect, useRef } from 'react';
import { buildMenuEntries } from '../contextMenu';
import { useDiscardPile } from '../discardPile';
import { useFileActions } from '../fileActions';
import { formatBytes, formatInt } from '../format';
import { canChangeDisk } from '../snapshotsModel';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { ContextMenu } from './ContextMenu';
import './contextMenu.css';

const NOTICE_MS = 6000;

/** Renders the context menu, the trash confirmation and the feedback line of file actions. */
export function FileActionsHost() {
  const menu = useFileActions((s) => s.menu);
  const confirm = useFileActions((s) => s.confirm);
  const notice = useFileActions((s) => s.notice);
  const isMac = useAppStore((s) => s.isMac);
  const rootId = useAppStore((s) => s.rootId);
  const readOnly = useAppStore((s) => !canChangeDisk(s.resultSource));
  const menuQueued = useDiscardPile((s) => (menu ? s.idSet.has(menu.details.node.id) : false));
  const confirmingBatch = useDiscardPile((s) => s.confirming);
  const outcome = useDiscardPile((s) => s.outcome);

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => useFileActions.getState().setNotice(''), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [notice]);

  // The result of a batch also goes to the notice line: the dock may be an icon rail.
  useEffect(() => {
    if (!outcome) return;
    useFileActions.getState().setNotice(
      outcome.error
        ? strings.discardBatchFailed(outcome.error)
        : strings.discardResult(
            formatInt(outcome.moved),
            formatBytes(outcome.bytes),
            formatInt(outcome.failed.length),
            outcome.moved === 1,
          ),
    );
  }, [outcome]);

  return (
    <>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          label={strings.fileActionsLabel}
          entries={buildMenuEntries(menu.details, {
            isMac,
            isRoot: menu.details.node.id === rootId,
            queued: menuQueued,
            readOnly,
          })}
          onSelect={(action) => useFileActions.getState().runMenuAction(action)}
          onClose={() => useFileActions.getState().closeMenu()}
        />
      )}
      {confirm && <ConfirmTrashDialog />}
      {confirmingBatch && <ConfirmDiscardDialog />}
      <div className="action-notice" role="status" hidden={!notice}>{notice}</div>
    </>
  );
}

export interface ConfirmProps {
  /** Prefix of the element ids the dialog uses for its title and body. */
  idBase: string;
  title: string;
  busy: boolean;
  confirmLabel: string;
  onCancel(): void;
  onConfirm(): void;
  children: React.ReactNode;
}

/** Modal that asks before something is moved to the Recycle Bin / Trash; Cancel has the focus. */
export function ConfirmDialog({ idBase, title, busy, confirmLabel, onCancel, onConfirm, children }: ConfirmProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const okRef = useRef<HTMLButtonElement>(null);

  // Default focus on Cancel; give the focus back to the trigger afterwards.
  useEffect(() => {
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    cancelRef.current?.focus();
    return () => {
      if (trigger?.isConnected) trigger.focus();
    };
  }, []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    } else if (e.key === 'Tab') {
      // Two buttons only: keep the focus inside the dialog.
      e.preventDefault();
      const buttons = [cancelRef.current, okRef.current].filter(
        (b): b is HTMLButtonElement => !!b && !b.disabled,
      );
      if (buttons.length === 0) return;
      const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
      const next = e.shiftKey ? (at <= 0 ? buttons.length - 1 : at - 1) : (at + 1) % buttons.length;
      buttons[next].focus();
    }
  };

  return (
    <div className="modal-backdrop" onKeyDown={onKeyDown}>
      <div
        className="modal"
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={`${idBase}-title`}
        aria-describedby={`${idBase}-body`}
      >
        <h2 id={`${idBase}-title`}>{title}</h2>
        <div id={`${idBase}-body`}>{children}</div>
        <div className="modal-actions">
          <button type="button" ref={cancelRef} className="btn" disabled={busy}
            onClick={onCancel}>{strings.cancel}</button>
          <button type="button" ref={okRef} className="btn danger" disabled={busy}
            onClick={onConfirm}>{busy ? strings.working : confirmLabel}</button>
        </div>
      </div>
    </div>
  );
}

/** Asks before one node is moved to the Recycle Bin / Trash. */
function ConfirmTrashDialog() {
  const confirm = useFileActions((s) => s.confirm)!;
  const isMac = useAppStore((s) => s.isMac);
  const { node } = confirm.details;
  const bin = strings.trashBinName(isMac);
  return (
    <ConfirmDialog
      idBase="trash"
      title={strings.confirmTrashTitle(bin)}
      busy={confirm.busy}
      confirmLabel={isMac ? strings.moveToTrash : strings.moveToRecycleBin}
      onCancel={() => useFileActions.getState().cancelTrash()}
      onConfirm={() => void useFileActions.getState().confirmTrash()}
    >
      <p className="modal-name">{node.name}</p>
      <p>{strings.confirmTrashSize(formatBytes(node.size))}</p>
      {node.is_dir && (
        <p>{strings.confirmTrashFolderContents(formatInt(node.files), formatInt(node.dirs))}</p>
      )}
      <p>{strings.confirmTrashBody(bin)}</p>
    </ConfirmDialog>
  );
}

/** Asks before the whole Discard Pile is moved to the Recycle Bin / Trash. */
function ConfirmDiscardDialog() {
  const summary = useDiscardPile((s) => s.summary);
  const busy = useDiscardPile((s) => s.busy);
  const isMac = useAppStore((s) => s.isMac);
  const bin = strings.trashBinName(isMac);
  const count = summary?.count ?? 0;
  return (
    <ConfirmDialog
      idBase="discard"
      title={strings.discardConfirmTitle(formatInt(count), bin, count === 1)}
      busy={busy}
      confirmLabel={isMac ? strings.moveToTrash : strings.moveToRecycleBin}
      onCancel={() => useDiscardPile.getState().cancelMoveAll()}
      onConfirm={() => void useDiscardPile.getState().confirmMoveAll()}
    >
      <p className="modal-name">{strings.discardSummary(formatInt(count), formatBytes(summary?.bytes ?? 0), count === 1)}</p>
      <p>{strings.discardConfirmBody(bin)}</p>
    </ConfirmDialog>
  );
}
