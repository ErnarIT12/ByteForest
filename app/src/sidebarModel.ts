import type { UserFolder } from './api';

export const SIDEBAR_COLLAPSED_KEY = 'byteforest.sidebarCollapsed';

/** The stored collapse flag; anything other than "1" means expanded. */
export function parseSidebarCollapsed(raw: string | null): boolean {
  return raw === '1';
}

export function serializeSidebarCollapsed(collapsed: boolean): string {
  return collapsed ? '1' : '0';
}

/** Order the quick locations are shown in. */
const FOLDER_ORDER = ['desktop', 'downloads', 'documents'];

/**
 * The user folders in display order (Desktop, Downloads, Documents, then anything else as
 * received). Entries without a path are dropped, as are repeated paths.
 */
export function orderUserFolders(folders: readonly UserFolder[]): UserFolder[] {
  const rank = (kind: string) => {
    const at = FOLDER_ORDER.indexOf(kind);
    return at < 0 ? FOLDER_ORDER.length : at;
  };
  const seen = new Set<string>();
  return folders
    .filter((folder) => {
      if (folder.path === '' || seen.has(folder.path)) return false;
      seen.add(folder.path);
      return true;
    })
    .map((folder, index) => ({ folder, index }))
    .sort((a, b) => rank(a.folder.kind) - rank(b.folder.kind) || a.index - b.index)
    .map(({ folder }) => folder);
}
