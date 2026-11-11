/** Full paths of nodes for tooltips, fetched once per scan result. */
import { getPath } from './api';

const cache = new Map<string, string>();
const inflight = new Map<string, Promise<string>>();
let cachedGeneration = -1;

function key(generation: number, id: number): string {
  if (generation !== cachedGeneration) {
    cache.clear();
    inflight.clear();
    cachedGeneration = generation;
  }
  return `${generation}:${id}`;
}

/** The path of `id` if it was already fetched for this result. */
export function cachedPath(generation: number, id: number): string | undefined {
  return cache.get(key(generation, id));
}

/** Fetches (once) the path of `id`. */
export function loadPath(generation: number, id: number): Promise<string> {
  const k = key(generation, id);
  const hit = cache.get(k);
  if (hit !== undefined) return Promise.resolve(hit);
  let p = inflight.get(k);
  if (!p) {
    p = getPath(id).then((path) => {
      if (cachedGeneration === generation) cache.set(k, path);
      inflight.delete(k);
      return path;
    });
    inflight.set(k, p);
  }
  return p;
}
