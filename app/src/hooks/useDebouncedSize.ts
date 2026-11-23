import { useEffect, useState, type RefObject } from 'react';

export interface Size {
  w: number;
  h: number;
  dpr: number;
}

const RESIZE_DEBOUNCE_MS = 100;

/** Measures the element's CSS size; the first size is immediate, later ones debounced. */
export function useDebouncedSize(ref: RefObject<HTMLDivElement | null>): Size | null {
  const [size, setSize] = useState<Size | null>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    let timer: number | undefined;
    let first = true;
    const measure = () => {
      const next = {
        w: Math.floor(el.clientWidth),
        h: Math.floor(el.clientHeight),
        dpr: window.devicePixelRatio || 1,
      };
      // Keep the same object when nothing changed so no refetch is triggered.
      const apply = () =>
        setSize((prev) =>
          prev && prev.w === next.w && prev.h === next.h && prev.dpr === next.dpr ? prev : next,
        );
      window.clearTimeout(timer);
      if (first) {
        first = false;
        apply();
      } else {
        timer = window.setTimeout(apply, RESIZE_DEBOUNCE_MS);
      }
    };
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    // Moving to a monitor with another scale changes devicePixelRatio and fires resize.
    window.addEventListener('resize', measure);
    return () => {
      ro.disconnect();
      window.removeEventListener('resize', measure);
      window.clearTimeout(timer);
    };
  }, [ref]);
  return size;
}
