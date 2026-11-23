/**
 * State and flows of the file actions: the context menu, the confirmation dialog and the
 * feedback message. IPC lives here; the menu and dialog components only render it.
 */
import { create } from 'zustand';
import {
  copyNodePath,
  locateNode,
  moveNodeToTrash,
  nodeDetails,
  revealNodeInFileManager,
  type NodeDetails,
} from './api';
import type { MenuAction } from './contextMenu';
import { useDiscardPile } from './discardPile';
import { canChangeDisk } from './snapshotsModel';
import { useAppStore } from './store';
import { strings } from './strings';

interface MenuState {
  scanId: number;
  details: NodeDetails;
  x: number;
  y: number;
}

interface ConfirmState {
  scanId: number;
  details: NodeDetails;
  busy: boolean;
}

interface FileActionsState {
  menu: MenuState | null;
  confirm: ConfirmState | null;
  /** Feedback line (success or failure) of the last action. */
  notice: string;
  /** Distinguishes overlapping `openMenu` lookups; only the latest one opens. */
  request: number;
  /** Looks the node up and opens the context menu at viewport position (`x`, `y`). */
  openMenu(scanId: number, id: number, x: number, y: number): Promise<void>;
  closeMenu(): void;
  runMenuAction(action: MenuAction): void;
  /** Opens the confirmation dialog for moving `id` to the Recycle Bin / Trash. */
  requestTrash(scanId: number, id: number): Promise<void>;
  cancelTrash(): void;
  confirmTrash(): Promise<void>;
  setNotice(message: string): void;
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

export const useFileActions = create<FileActionsState>()((set, get) => {
  /** Runs a path based IPC action and reports the outcome in the notice line. */
  const perform = async (
    scanId: number,
    id: number,
    action: (scan: number, node: number) => Promise<void>,
    success = '',
  ) => {
    try {
      await action(scanId, id);
      set({ notice: success });
    } catch (error) {
      set({ notice: message(error) });
    }
  };

  return {
    menu: null,
    confirm: null,
    notice: '',
    request: 0,

    openMenu: async (scanId, id, x, y) => {
      const request = get().request + 1;
      set({ request, notice: '' });
      try {
        const details = await nodeDetails(scanId, id);
        if (get().request !== request || useAppStore.getState().resultScanId !== scanId) return;
        set({ menu: { scanId, details, x, y } });
      } catch (error) {
        if (get().request === request) set({ notice: message(error) });
      }
    },

    closeMenu: () => set((s) => ({ menu: null, request: s.request + 1 })),

    runMenuAction: (action) => {
      const menu = get().menu;
      if (!menu) return;
      const { scanId, details } = menu;
      const id = details.node.id;
      set({ menu: null });
      switch (action) {
        case 'open':
          void locateNode(scanId, id).then(
            (steps) => {
              if (useAppStore.getState().resultScanId !== scanId) return;
              useAppStore
                .getState()
                .setFocus(id, steps.map(({ id: stepId, name }) => ({ id: stepId, name })));
            },
            (error: unknown) => set({ notice: message(error) }),
          );
          break;
        case 'reveal':
          void perform(scanId, id, revealNodeInFileManager);
          break;
        case 'copy':
          void perform(scanId, id, copyNodePath, strings.pathCopied);
          break;
        case 'discardAdd':
          if (useAppStore.getState().resultScanId === scanId) useDiscardPile.getState().add([id]);
          break;
        case 'discardRemove':
          if (useAppStore.getState().resultScanId === scanId) useDiscardPile.getState().remove(id);
          break;
        case 'trash':
          if (!canChangeDisk(useAppStore.getState().resultSource)) {
            set({ notice: strings.snapshotReadOnly });
            break;
          }
          set({ confirm: { scanId, details, busy: false } });
          break;
      }
    },

    requestTrash: async (scanId, id) => {
      if (!canChangeDisk(useAppStore.getState().resultSource)) {
        set({ notice: strings.snapshotReadOnly });
        return;
      }
      set({ notice: '' });
      try {
        const details = await nodeDetails(scanId, id);
        if (useAppStore.getState().resultScanId !== scanId) return;
        set({ menu: null, confirm: { scanId, details, busy: false } });
      } catch (error) {
        set({ notice: message(error) });
      }
    },

    cancelTrash: () => {
      if (!get().confirm?.busy) set({ confirm: null });
    },

    confirmTrash: async () => {
      const confirm = get().confirm;
      if (!confirm || confirm.busy) return;
      const { scanId, details } = confirm;
      const bin = strings.trashBinName(useAppStore.getState().isMac);
      set({ confirm: { ...confirm, busy: true } });
      try {
        const result = await moveNodeToTrash(scanId, details.node.id);
        if (useAppStore.getState().resultScanId === scanId) {
          useAppStore.getState().applyRemoval(details.node.id, result);
        }
        set({ confirm: null, notice: strings.movedToTrash(details.node.name, bin) });
      } catch (error) {
        set({ confirm: null, notice: strings.trashFailed(message(error)) });
      }
    },

    setNotice: (notice) => set({ notice }),
  };
});
