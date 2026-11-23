import { useEffect, useRef } from 'react';
import { buildMenuEntries } from '../contextMenu';
import { useFileActions } from '../fileActions';
import { formatBytes, formatInt } from '../format';
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

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => useFileActions.getState().setNotice(''), NOTICE_MS);
    return () => window.clearTimeout(timer);
  }, [notice]);

  return (
    <>
      {menu && (
        <ContextMenu
          x={menu.x}
          y={menu.y}
          label={strings.fileActionsLabel}
          entries={buildMenuEntries(menu.details, { isMac, isRoot: menu.details.node.id === rootId })}
          onSelect={(action) => useFileActions.getState().runMenuAction(action)}
          onClose={() => useFileActions.getState().closeMenu()}
        />
      )}
      {confirm && <ConfirmTrashDialog />}
      <div className="action-notice" role="status" hidden={!notice}>{notice}</div>
    </>
  );
}

/** Modal that asks before a node is moved to the Recycle Bin / Trash. */
function ConfirmTrashDialog() {
  const confirm = useFileActions((s) => s.confirm)!;
  const isMac = useAppStore((s) => s.isMac);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const okRef = useRef<HTMLButtonElement>(null);
  const { node } = confirm.details;
  const bin = strings.trashBinName(isMac);

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
      useFileActions.getState().cancelTrash();
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
        aria-labelledby="trash-title"
        aria-describedby="trash-body"
      >
        <h2 id="trash-title">{strings.confirmTrashTitle(bin)}</h2>
        <div id="trash-body">
          <p className="modal-name">{node.name}</p>
          <p>{strings.confirmTrashSize(formatBytes(node.size))}</p>
          {node.is_dir && (
            <p>{strings.confirmTrashFolderContents(formatInt(node.files), formatInt(node.dirs))}</p>
          )}
          <p>{strings.confirmTrashBody(bin)}</p>
        </div>
        <div className="modal-actions">
          <button type="button" ref={cancelRef} className="btn" disabled={confirm.busy}
            onClick={() => useFileActions.getState().cancelTrash()}>{strings.cancel}</button>
          <button type="button" ref={okRef} className="btn danger" disabled={confirm.busy}
            onClick={() => void useFileActions.getState().confirmTrash()}>
            {confirm.busy ? strings.working : isMac ? strings.moveToTrash : strings.moveToRecycleBin}
          </button>
        </div>
      </div>
    </div>
  );
}
