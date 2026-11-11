import { useSyncExternalStore } from 'react';
import type { Theme } from '../treemapPaint';

const query = '(prefers-color-scheme: dark)';

function subscribe(onChange: () => void): () => void {
  const mql = window.matchMedia(query);
  mql.addEventListener('change', onChange);
  return () => mql.removeEventListener('change', onChange);
}

/** The system light / dark preference, updated live. */
export function useColorScheme(): Theme {
  return useSyncExternalStore(subscribe, () => (window.matchMedia(query).matches ? 'dark' : 'light'));
}
