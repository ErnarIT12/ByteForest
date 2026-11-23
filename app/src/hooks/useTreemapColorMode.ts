import { useSyncExternalStore } from 'react';

/** `depth`: colour by top-level folder; `category`: colour files by file type. */
export type ColorMode = 'depth' | 'category';

const key = 'byteforest.treemapColors';
const event = 'byteforest:treemapColors';
let mode: ColorMode = 'depth';

try {
  const stored = localStorage.getItem(key);
  if (stored === 'depth' || stored === 'category') mode = stored;
} catch {
  /* Storage can be unavailable; the mode then lasts for this session only. */
}

export function getTreemapColorMode(): ColorMode {
  return mode;
}

export function setTreemapColorMode(value: ColorMode): void {
  mode = value;
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Apply for this session. */
  }
  window.dispatchEvent(new Event(event));
}

function subscribe(callback: () => void) {
  window.addEventListener(event, callback);
  return () => window.removeEventListener(event, callback);
}

export function useTreemapColorMode(): ColorMode {
  return useSyncExternalStore(subscribe, getTreemapColorMode);
}
