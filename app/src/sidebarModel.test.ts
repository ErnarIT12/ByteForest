import { describe, expect, it } from 'vitest';
import type { UserFolder } from './api';
import { orderUserFolders, parseSidebarCollapsed, serializeSidebarCollapsed } from './sidebarModel';

const folder = (kind: string, path = `/home/${kind}`): UserFolder => ({ kind, name: kind, path });

describe('sidebar collapse flag', () => {
  it('defaults to expanded', () => {
    expect(parseSidebarCollapsed(null)).toBe(false);
    expect(parseSidebarCollapsed('')).toBe(false);
    expect(parseSidebarCollapsed('true')).toBe(false);
  });

  it('round-trips', () => {
    expect(parseSidebarCollapsed(serializeSidebarCollapsed(true))).toBe(true);
    expect(parseSidebarCollapsed(serializeSidebarCollapsed(false))).toBe(false);
  });
});

describe('orderUserFolders', () => {
  it('puts Desktop, Downloads, Documents in that order', () => {
    const ordered = orderUserFolders([folder('documents'), folder('desktop'), folder('downloads')]);
    expect(ordered.map((f) => f.kind)).toEqual(['desktop', 'downloads', 'documents']);
  });

  it('keeps unknown kinds last in their given order', () => {
    const ordered = orderUserFolders([folder('music'), folder('documents'), folder('pictures')]);
    expect(ordered.map((f) => f.kind)).toEqual(['documents', 'music', 'pictures']);
  });

  it('drops empty and repeated paths', () => {
    const ordered = orderUserFolders([
      folder('desktop', ''),
      folder('downloads', '/x'),
      folder('documents', '/x'),
    ]);
    expect(ordered.map((f) => f.kind)).toEqual(['downloads']);
  });
});
