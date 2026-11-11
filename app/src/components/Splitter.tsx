import { useRef, type KeyboardEvent, type PointerEvent } from 'react';
import { strings } from '../strings';

interface Props {
  /** Share of the container given to the left pane, 0..1. */
  value: number;
  min: number;
  max: number;
  onChange(value: number): void;
}

/** Vertical drag handle between the tree and the treemap; arrow keys move it too. */
export function Splitter({ value, min, max, onChange }: Props) {
  const dragging = useRef(false);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    dragging.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    if (!dragging.current) return;
    const box = e.currentTarget.parentElement?.getBoundingClientRect();
    if (box && box.width > 0) onChange(clamp((e.clientX - box.left) / box.width));
  };
  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    dragging.current = false;
    if (e.currentTarget.hasPointerCapture(e.pointerId)) {
      e.currentTarget.releasePointerCapture(e.pointerId);
    }
  };
  /** Cancelled pointer or capture lost (window blur, touch cancel): stop dragging. */
  const stopDrag = () => {
    dragging.current = false;
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'ArrowLeft') onChange(clamp(value - 0.02));
    else if (e.key === 'ArrowRight') onChange(clamp(value + 0.02));
    else return;
    e.preventDefault();
  };

  return (
    <div
      className="splitter"
      role="separator"
      aria-orientation="vertical"
      aria-label={strings.splitterLabel}
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
