import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
  type MouseEvent,
  type WheelEvent,
} from 'react';
import { getNode, getSunburst } from '../api';
import { deleteKeyIntent, isContextMenuKey } from '../contextMenu';
import { guardWritable } from '../readOnly';
import { useDiscardPile } from '../discardPile';
import { useFileActions } from '../fileActions';
import { formatBytes } from '../format';
import { useColorScheme } from '../hooks/useColorScheme';
import { useDebouncedSize } from '../hooks/useDebouncedSize';
import { useTreemapColorMode } from '../hooks/useTreemapColorMode';
import { useAppStore } from '../store';
import { strings } from '../strings';
import { largestChildArc, parentArc, siblingArc } from '../sunburstNav';
import {
  arcRadii,
  buildSunburstLayout,
  isNodeArc,
  paintSunburst,
  paintSunburstHighlights,
  sunburstCrumbsTo,
  sunburstHitTest,
  type SunburstHit,
  type SunburstLayout,
} from '../sunburstPaint';
import { TreemapTooltip } from './TreemapTooltip';

/** Wheel steps closer together than this count as one "go up". */
const WHEEL_GAP_MS = 350;

type Hover = { hit: SunburstHit; x: number; y: number } | null;

/** The sunburst of the focused folder, drawn on two stacked canvases like the treemap. */
export function SunburstCanvas() {
  const focusId = useAppStore((s) => s.focusId);
  const scanId = useAppStore((s) => s.resultScanId);
  const selectedId = useAppStore((s) => s.selectedId);
  const generation = useAppStore((s) => s.generation);
  const baselineSeq = useAppStore((s) => s.baselineSeq);
  const comparing = useAppStore((s) => s.baseline !== null);
  const canGoUp = useAppStore((s) => s.focusPath.length > 1);
  const queuedIds = useDiscardPile((s) => s.idSet);
  const theme = useColorScheme();
  const colorMode = useTreemapColorMode();

  const wrapRef = useRef<HTMLDivElement>(null);
  const baseRef = useRef<HTMLCanvasElement>(null);
  const overlayRef = useRef<HTMLCanvasElement>(null);
  const lastWheel = useRef(0);
  const size = useDebouncedSize(wrapRef);
  const [fetched, setFetched] = useState<{ layout: SunburstLayout; generation: number } | null>(
    null,
  );
  // A layout of a previous scan result is never shown, clicked or hovered.
  const layout = fetched && fetched.generation === generation ? fetched.layout : null;
  const [failed, setFailed] = useState<string | null>(null);
  const [hover, setHover] = useState<Hover>(null);

  // Fetch arcs whenever the focus, the result, the comparison or the canvas size changes.
  useEffect(() => {
    if (focusId === null || scanId === null || !size || size.w < 8 || size.h < 8) return;
    let alive = true;
    const width = Math.round(size.w * size.dpr);
    const height = Math.round(size.h * size.dpr);
    Promise.all([getSunburst(scanId, focusId), getNode(focusId)]).then(
      ([arcs, node]) => {
        if (!alive) return;
        setFailed(null);
        setHover(null);
        setFetched({
          layout: buildSunburstLayout(
            arcs,
            { id: node.id, name: node.name, size: node.size },
            size.dpr,
            width,
            height,
          ),
          generation,
        });
      },
      (e) => alive && setFailed(String(e)),
    );
    return () => {
      alive = false;
    };
  }, [focusId, scanId, size, generation, baselineSeq]);

  // Base layer: every arc, repainted only for a new layout, theme or colour mode.
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
    const css = getComputedStyle(canvas);
    const token = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback;
    paintSunburst(
      ctx,
      layout,
      theme,
      colorMode,
      {
        background: token('--tm-bg', '#808080'),
        centerFill: token('--surface-raised', '#303030'),
        ink: token('--text', '#ffffff'),
        inkSub: token('--text-2', '#cccccc'),
      },
      comparing
        ? {
            grow: token('--delta-grow', '#9a3412'),
            shrink: token('--delta-shrink', '#17663a'),
            halo: token('--tm-sel-inner', '#000000'),
          }
        : null,
    );
  }, [layout, theme, colorMode, comparing]);

  // Overlay: hover, selection and Discard Pile markers.
  const hoverIndex = hover?.hit.zone === 'arc' ? hover.hit.index : -1;
  const hoverCenter = hover?.hit.zone === 'center' && canGoUp;
  const selectedIndex = layout && selectedId !== null ? (layout.index.get(selectedId) ?? -1) : -1;
  const queuedIndexes = useMemo(() => {
    if (!layout) return [];
    const found: number[] = [];
    for (const id of queuedIds) {
      const i = layout.index.get(id);
      if (i !== undefined) found.push(i);
    }
    return found;
  }, [layout, queuedIds]);
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
    paintSunburstHighlights(ctx, layout, hoverIndex, hoverCenter, selectedIndex, queuedIndexes, {
      accent: token('--accent', '#5edb7f'),
      hoverFill: token('--tm-hover-fill', 'rgba(255,255,255,0.08)'),
      hoverLine: token('--tm-hover-line', 'rgba(206,251,246,0.85)'),
      selectedInner: token('--tm-sel-inner', '#000000'),
      queued: token('--tm-discard', '#7a1626'),
    });
  }, [layout, hoverIndex, hoverCenter, selectedIndex, queuedIndexes, theme]);

  const hitAt = (e: MouseEvent<HTMLElement>): SunburstHit => {
    if (!layout) return { zone: 'none' };
    const box = e.currentTarget.getBoundingClientRect();
    return sunburstHitTest(layout, (e.clientX - box.left) * layout.dpr, (e.clientY - box.top) * layout.dpr);
  };

  /** The layout of the previous focus can still be on screen while the new one loads. */
  const layoutIsCurrent = (l: SunburstLayout) => {
    const { focusPath } = useAppStore.getState();
    return focusPath[focusPath.length - 1]?.id === l.root.id;
  };

  const onMouseMove = (e: MouseEvent<HTMLDivElement>) => {
    const hit = hitAt(e);
    const box = e.currentTarget.getBoundingClientRect();
    setHover(
      hit.zone === 'none' ? null : { hit, x: e.clientX - box.left, y: e.clientY - box.top },
    );
  };

  /** Selects arc `i` exactly as a single click would. */
  const revealIndex = (i: number) => {
    if (!layout || !isNodeArc(layout, i) || !layoutIsCurrent(layout)) return;
    const { focusPath } = useAppStore.getState();
    useAppStore.getState().revealNode(sunburstCrumbsTo(layout, i, focusPath).map((c) => c.id));
  };

  const onClick = (e: MouseEvent<HTMLDivElement>) => {
    const hit = hitAt(e);
    if (!layout) return;
    if (hit.zone === 'center') {
      useAppStore.getState().goUp();
      return;
    }
    if (hit.zone !== 'arc' || !isNodeArc(layout, hit.index) || !layoutIsCurrent(layout)) return;
    const arc = layout.arcs[hit.index];
    const { focusPath } = useAppStore.getState();
    const path = sunburstCrumbsTo(layout, hit.index, focusPath);
    if (arc.is_dir && e.detail === 2) useAppStore.getState().setFocus(arc.id, path);
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

  /** Right click: selects the arc under the pointer, then opens the menu. */
  const onContextMenu = (e: MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    const hit = hitAt(e);
    if (!layout || hit.zone !== 'arc' || !isNodeArc(layout, hit.index)) return;
    revealIndex(hit.index);
    const { resultScanId } = useAppStore.getState();
    if (resultScanId !== null) {
      void useFileActions
        .getState()
        .openMenu(resultScanId, layout.arcs[hit.index].id, e.clientX, e.clientY);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (isContextMenuKey(e) && e.target === e.currentTarget) {
      e.preventDefault();
      const { resultScanId } = useAppStore.getState();
      if (!layout || !isNodeArc(layout, selectedIndex) || resultScanId === null) return;
      const { r0, r1 } = arcRadii(layout, selectedIndex);
      const a = layout.arcs[selectedIndex];
      const m = (a.start + a.end) / 2 - Math.PI / 2;
      const mid = (r0 + r1) / 2;
      const box = e.currentTarget.getBoundingClientRect();
      void useFileActions.getState().openMenu(
        resultScanId,
        a.id,
        box.left + (layout.cx + mid * Math.cos(m)) / layout.dpr,
        box.top + (layout.cy + mid * Math.sin(m)) / layout.dpr,
      );
      return;
    }
    const intent = deleteKeyIntent(e);
    if (intent && e.target === e.currentTarget) {
      // Delete only queues; Shift+Delete asks before moving to the Recycle Bin / Trash.
      const { resultScanId } = useAppStore.getState();
      if (!layout || !isNodeArc(layout, selectedIndex) || resultScanId === null) return;
      e.preventDefault();
      const id = layout.arcs[selectedIndex].id;
      if (!guardWritable()) return;
      if (intent === 'queue') useDiscardPile.getState().add([id]);
      else void useFileActions.getState().requestTrash(resultScanId, id);
      return;
    }
    if (e.ctrlKey || e.altKey || e.metaKey || e.target !== e.currentTarget) return;
    if (e.key === 'Escape' || e.key === 'Backspace') {
      e.preventDefault();
      useAppStore.getState().goUp();
      return;
    }
    if (!layout) return;
    if (['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) {
      e.preventDefault();
      // A selection outside this layout counts as no selection.
      if (selectedIndex < 0) revealIndex(largestChildArc(layout, -1));
      else if (e.key === 'ArrowLeft') revealIndex(siblingArc(layout, selectedIndex, -1));
      else if (e.key === 'ArrowRight') revealIndex(siblingArc(layout, selectedIndex, 1));
      else if (e.key === 'ArrowUp') revealIndex(parentArc(layout, selectedIndex));
      else revealIndex(largestChildArc(layout, selectedIndex));
    } else if (e.key === 'Enter' || e.code === 'NumpadEnter') {
      e.preventDefault();
      if (selectedIndex < 0) {
        revealIndex(largestChildArc(layout, -1));
        return;
      }
      const a = layout.arcs[selectedIndex];
      if (a.is_dir && isNodeArc(layout, selectedIndex) && layoutIsCurrent(layout)) {
        const { focusPath } = useAppStore.getState();
        useAppStore.getState().setFocus(a.id, sunburstCrumbsTo(layout, selectedIndex, focusPath));
      }
    }
  };

  const selectedArc = layout && selectedIndex >= 0 ? layout.arcs[selectedIndex] : null;
  const cssW = layout ? layout.width / layout.dpr : 0;
  const cssH = layout ? layout.height / layout.dpr : 0;
  const hovered = layout && hover?.hit.zone === 'arc' ? layout.arcs[hover.hit.index] : null;

  return (
    <div
      ref={wrapRef}
      className="treemap"
      data-ready={layout ? true : undefined}
      role="group"
      aria-roledescription={strings.sunburstRoleDescription}
      aria-label={strings.sunburstLabel}
      tabIndex={0}
      onKeyDown={onKeyDown}
      onMouseMove={onMouseMove}
      onMouseLeave={() => setHover(null)}
      onClick={onClick}
      onContextMenu={onContextMenu}
      onWheel={onWheel}
      onMouseUp={onMouseUp}
      style={{ cursor: hovered || hoverCenter ? 'pointer' : 'default' }}
    >
      <canvas ref={baseRef} className="treemap-layer" style={{ width: cssW, height: cssH }} />
      <canvas ref={overlayRef} className="treemap-layer" style={{ width: cssW, height: cssH }} />
      <div className="sr-only" aria-live="polite">
        {selectedArc &&
          strings.treemapSelected(
            selectedArc.name,
            selectedArc.is_dir ? strings.folder : strings.file,
            formatBytes(selectedArc.size),
          )}
      </div>
      {layout && layout.arcs.length === 0 && (
        <div className="treemap-note">{strings.treemapEmpty}</div>
      )}
      {failed && <div className="treemap-note">{failed}</div>}
      {hovered && hover && size && layout && (
        <TreemapTooltip
          rect={hovered}
          generation={generation}
          viewSize={layout.root.size}
          x={hover.x}
          y={hover.y}
          flipX={hover.x > size.w * 0.6}
          flipY={hover.y > size.h * 0.6}
        />
      )}
    </div>
  );
}
