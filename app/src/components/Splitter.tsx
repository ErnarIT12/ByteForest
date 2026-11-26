import { useRef, type KeyboardEvent, type PointerEvent } from 'react';
import { strings } from '../strings';

interface Props {
  /** Share of the container given to the left pane, 0..1. */
  value: number;
  horizontal?: boolean;
  min: number;
  max: number;
  onChange(value: number): void;
}

/** Vertical drag handle between the treemap and the tree; arrow keys move it too. */
export function Splitter({ value, min, max, onChange, horizontal = false }: Props) {
  const drag = useRef<{ start: number; share: number; length: number } | null>(null);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    const parent = e.currentTarget.parentElement;
    if (!parent) return;
    const length = horizontal
      ? parent.clientHeight - (parent.querySelector('header')?.offsetHeight ?? 0) - 8
      : parent.clientWidth - 8;
    drag.current = { start: horizontal ? e.clientY : e.clientX, share: value, length: Math.max(1, length) };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!drag.current) return;
    const { start, share, length } = drag.current;
    onChange(clamp(share + ((horizontal ? e.clientY : e.clientX) - start) / length));
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    drag.current = null;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };
  /** Cancelled pointer or capture lost (window blur, touch cancel): stop dragging. */
  const stopDrag = () => {
    drag.current = null;
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === (horizontal ? 'ArrowUp' : 'ArrowLeft')) onChange(clamp(value - 0.02));
    else if (e.key === (horizontal ? 'ArrowDown' : 'ArrowRight')) onChange(clamp(value + 0.02));
    else return;
    e.preventDefault();
  };

  return (
    <div
      className={horizontal ? "splitter row-splitter" : "splitter"}
      role="separator"
      aria-orientation={horizontal ? "horizontal" : "vertical"}
      aria-label={horizontal ? "Resize file tree and details" : strings.splitterLabel}
      aria-valuemin={Math.round(min * 100)}
      aria-valuemax={Math.round(max * 100)}
      aria-valuenow={Math.round(value * 100)}
      tabIndex={0}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerCancel={stopDrag}
      onLostPointerCapture={stopDrag}
      onKeyDown={onKeyDown}
    />
  );
}
