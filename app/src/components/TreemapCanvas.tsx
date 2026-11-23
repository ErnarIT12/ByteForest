import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type RefObject,
  type WheelEvent,
} from 'react';
import { getTreemap } from '../api';
import { formatBytes } from '../format';
import { useColorScheme } from '../hooks/useColorScheme';
import { useAppStore } from '../store';
import { strings } from '../strings';
import {
  HEADER_CSS_PX,
  buildLayout,
  crumbsTo,
  hitTest,
  paintHighlights,
  paintTreemap,
  type TreemapLayout,
} from '../treemapPaint';
import { largestChild, nearestSibling, type Direction } from '../treemapNav';
import { TreemapTooltip } from './TreemapTooltip';

interface Size {
  w: number;
  h: number;
  dpr: number;
}

/** Wheel steps closer together than this count as one "go up". */
const WHEEL_GAP_MS = 350;
const RESIZE_DEBOUNCE_MS = 100;

/** Measures the element's CSS size; the first size is immediate, later ones debounced. */
function useDebouncedSize(ref: RefObject<HTMLDivElement | null>): Size | null {
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

/** The squarified treemap of the focused folder, drawn on two stacked canvases. */
export function TreemapCanvas() {
  const focusId = useAppStore((s) => s.focusId);
  const selectedId = useAppStore((s) => s.selectedId);
  const searchFilename = useAppStore((s) =>
    s.reveal?.path.at(-1) === s.selectedId ? s.reveal?.filename : undefined,
  );
  const generation = useAppStore((s) => s.generation);
  const theme = useColorScheme();

  const wrapRef = useRef<HTMLDivElement>(null);
  const baseRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const lastWheel = useRef(0);
  const size = useDebouncedSize(wrapRef);
  /** The last fetched layout and the scan result it belongs to. */
  const [fetched, setFetched] = useState<{ layout: TreemapLayout; generation: number } | null>(
    null,
  );
  // A layout of a previous scan result is never shown, clicked or hovered.
  const layout = fetched && fetched.generation === generation ? fetched.layout : null;
  const [failed, setFailed] = useState<string | null>(null);
  const [hover, setHover] = useState<{ i: number; x: number; y: number } | null>(null);

  // Fetch rectangles whenever the focus, the result or the canvas size changes.
  useEffect(() => {
    if (focusId === null || !size || size.w < 8 || size.h < 8) return;
    let alive = true;
    const width = Math.round(size.w * size.dpr);
    const height = Math.round(size.h * size.dpr);
    const header = Math.round(HEADER_CSS_PX * size.dpr);
    getTreemap(focusId, width, height, header).then(
      (rects) => {
        if (!alive) return;
        setFailed(null);
        setHover(null);
        setFetched({ layout: buildLayout(rects, size.dpr, header, width, height), generation });
      },
      (e) => alive && setFailed(String(e)),
    );
    return () => {
      alive = false;
    };
  }, [focusId, size, generation]);

  // Base layer: every rectangle, repainted only for a new layout or theme.
  useEffect(() => {
    const canvas = baseRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    if (!layout) {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      return;
    }
    canvas.width = layout.width;
    canvas.height = layout.height;
    const background = getComputedStyle(canvas).getPropertyValue('--tm-bg').trim() || '#808080';
    paintTreemap(ctx, layout, theme, background);
  }, [layout, theme]);

  // Overlay: hover and selection outlines.
  const hoverIndex = hover?.i ?? -1;
  const selectedIndex = layout && selectedId !== null ? (layout.index.get(selectedId) ?? -1) : -1;
  const selectionBelowResolution = layout?.rects[0]?.id === focusId && searchFilename && selectedIndex < 0;
  useEffect(() => {
    const canvas = overlayRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    if (!layout) {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      return;
    }
    if (canvas.width !== layout.width) canvas.width = layout.width;
    if (canvas.height !== layout.height) canvas.height = layout.height;
    const css = getComputedStyle(canvas);
    const token = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
    paintHighlights(ctx, layout, hoverIndex, selectedIndex, {
      accent: token('--accent', '#5edb7f'),
      hoverFill: token('--tm-hover-fill', 'rgba(255,255,255,0.08)'),
      hoverLine: token('--tm-hover-line', 'rgba(206,251,246,0.85)'),
      selectedInner: token('--tm-sel-inner', '#000000'),
    });
  }, [layout, hoverIndex, selectedIndex, theme]);

  const indexAt = (e: MouseEvent<HTMLElement>): number => {
    if (!layout) return -1;
    const box = e.currentTarget.getBoundingClientRect();
    return hitTest(layout, (e.clientX - box.left) * layout.dpr, (e.clientY - box.top) * layout.dpr);
  };

  const onMouseMove = (e: MouseEvent<HTMLDivElement>) => {
    const i = indexAt(e);
    const box = e.currentTarget.getBoundingClientRect();
    setHover(i > 0 ? { i, x: e.clientX - box.left, y: e.clientY - box.top } : null);
  };

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    const i = indexAt(e);
    if (!layout || i <= 0) return;
    const { focusPath } = useAppStore.getState();
    // The layout of the previous focus can still be on screen while the new one loads.
    if (focusPath[focusPath.length - 1]?.id !== layout.rects[0].id) return;
    const r = layout.rects[i];
    const path = crumbsTo(layout, i, focusPath);
    if (r.is_dir && e.detail === 2) useAppStore.getState().setFocus(r.id, path);
    else useAppStore.getState().revealNode(path.map((c) => c.id));
  };

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    if (e.deltaY <= 0) return;
    const now = performance.now();
    if (now - lastWheel.current > WHEEL_GAP_MS) useAppStore.getState().goUp();
    lastWheel.current = now;
  };

  const onMouseUp = (e: MouseEvent<HTMLDivElement>) => {
    // Mouse "back" button.
    if (e.button === 3) {
      e.preventDefault();
      useAppStore.getState().goUp();
    }
  };

  /** Selects rect `i` exactly as a single click would. */
  const revealIndex = (i: number) => {
    if (!layout || i <= 0) return;
    const { focusPath } = useAppStore.getState();
    if (focusPath[focusPath.length - 1]?.id !== layout.rects[0].id) return;
    useAppStore.getState().revealNode(crumbsTo(layout, i, focusPath).map((c) => c.id));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.ctrlKey || e.altKey || e.metaKey || e.target !== e.currentTarget) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      useAppStore.getState().goUp();
      return;
    }
    if (!layout) return;
    const arrows: Record<string, Direction> = {
      ArrowLeft: 'left',
      ArrowRight: 'right',
      ArrowUp: 'up',
      ArrowDown: 'down',
    };
    const dir = arrows[e.key];
    if (dir) {
      e.preventDefault();
      // A selection outside this layout counts as no selection.
      revealIndex(
        selectedIndex > 0
          ? nearestSibling(layout.rects, selectedIndex, dir)
          : largestChild(layout.rects),
      );
    } else if (e.key === 'Enter' || e.code === 'NumpadEnter') {
      e.preventDefault();
      if (selectedIndex <= 0) {
        revealIndex(largestChild(layout.rects));
        return;
      }
      const r = layout.rects[selectedIndex];
      const { focusPath } = useAppStore.getState();
      if (r.is_dir && focusPath[focusPath.length - 1]?.id === layout.rects[0].id) {
        useAppStore.getState().setFocus(r.id, crumbsTo(layout, selectedIndex, focusPath));
      }
    }
  };

  const selectedRect = layout && selectedIndex > 0 ? layout.rects[selectedIndex] : null;
  const cssW = layout ? layout.width / layout.dpr : 0;
  const cssH = layout ? layout.height / layout.dpr : 0;
  const hovered = layout && hover ? layout.rects[hover.i] : null;

  return (
    <div
      ref={wrapRef}
      className="treemap"
      role="group"
      aria-roledescription={strings.treemapRoleDescription}
      aria-label={strings.treemapLabel}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onMouseMove={onMouseMove}
      onMouseLeave={() => setHover(null)}
      onClick={onClick}
      onWheel={onWheel}
      onMouseUp={onMouseUp}
      style={{ cursor: hover ? 'pointer' : 'default' }}
    >
      <canvas ref={baseRef} className="treemap-layer" style={{ width: cssW, height: cssH }} />
      <canvas ref={overlayRef} className="treemap-layer" style={{ width: cssW, height: cssH }} />
      <div className="sr-only" aria-live="polite">
        {selectedRect &&
          strings.treemapSelected(
            selectedRect.name,
            selectedRect.is_dir ? strings.folder : strings.file,
            formatBytes(selectedRect.size),
          )}
      </div>
      {selectionBelowResolution && (
        <div className="treemap-note" title={searchFilename}>Selected: {searchFilename}</div>
      )}
      {layout && layout.rects.length <= 1 && !selectionBelowResolution && (
        <div className="treemap-note">{strings.treemapEmpty}</div>
      )}
      {failed && <div className="treemap-note">{failed}</div>}
      {hovered && hover && size && layout && (
        <TreemapTooltip
          rect={hovered}
          generation={generation}
          viewSize={layout.rects[0]?.size ?? 0}
          x={hover.x}
          y={hover.y}
          flipX={hover.x > size.w * 0.6}
          flipY={hover.y > size.h * 0.6}
        />
      )}
    </div>
  );
}
