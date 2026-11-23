import { useSyncExternalStore } from 'react';
import type { Theme } from '../treemapPaint';

export type Appearance = Theme | 'system';
const key = 'byteforest.appearance';
const event = 'byteforest:appearance';
let preference: Appearance = 'dark';

export function getAppearance() { return preference; }

function applyAppearance() {
  const systemDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  document.documentElement.dataset.theme = preference === 'system' ? (systemDark ? 'dark' : 'light') : preference;
  window.dispatchEvent(new Event(event));
}

export function initializeAppearance() {
  try {
    const stored = localStorage.getItem(key);
    if (stored === 'system' || stored === 'light' || stored === 'dark') preference = stored;
  } catch { /* The current window can still set a theme without storage. */ }
  applyAppearance();
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', applyAppearance);
}

export function setAppearance(value: Appearance) {
  preference = value;
  try { localStorage.setItem(key, value); } catch { /* Apply for this session. */ }
  applyAppearance();
}

function subscribe(callback: () => void) {
  window.addEventListener(event, callback);
  return () => window.removeEventListener(event, callback);
}

export function useColorScheme(): Theme {
  return useSyncExternalStore(subscribe, () => document.documentElement.dataset.theme === 'dark' ? 'dark' : 'light');
}
