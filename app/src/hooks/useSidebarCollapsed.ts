import { useSyncExternalStore } from 'react';
import {
  SIDEBAR_COLLAPSED_KEY,
  parseSidebarCollapsed,
  serializeSidebarCollapsed,
} from '../sidebarModel';

const event = 'byteforest:sidebarCollapsed';
let collapsed = false;

try {
  collapsed = parseSidebarCollapsed(localStorage.getItem(SIDEBAR_COLLAPSED_KEY));
} catch {
  /* Storage can be unavailable; the sidebar then starts expanded each time. */
}

export function setSidebarCollapsed(value: boolean): void {
  collapsed = value;
  try {
    localStorage.setItem(SIDEBAR_COLLAPSED_KEY, serializeSidebarCollapsed(value));
  } catch {
    /* Apply for this session. */
  }
  window.dispatchEvent(new Event(event));
}

function subscribe(callback: () => void) {
  window.addEventListener(event, callback);
  return () => window.removeEventListener(event, callback);
}

/** Whether the user collapsed the sidebar to its icon rail (persisted). */
export function useSidebarCollapsed(): boolean {
  return useSyncExternalStore(subscribe, () => collapsed);
}
