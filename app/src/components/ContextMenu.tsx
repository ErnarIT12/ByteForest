import { useEffect, useLayoutEffect, useRef } from 'react';
import { clampMenuPosition, nextEnabledIndex, type MenuEntry, type MenuKey } from '../contextMenu';
import './contextMenu.css';

interface Props {
  x: number;
  y: number;
  label: string;
  entries: MenuEntry[];
  onSelect(action: Extract<MenuEntry, { type: 'item' }>['action']): void;
  onClose(): void;
}

const NAV_KEYS = new Set(['ArrowDown', 'ArrowUp', 'Home', 'End']);

/** A small keyboard accessible context menu positioned at a point of the viewport. */
export function ContextMenu({ x, y, label, entries, onSelect, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  useEffect(() => {
    onCloseRef.current = onClose;
  });
  const enabled = entries.flatMap((e) => (e.type === 'item' ? [!e.disabled] : []));

  // Keep the menu inside the window and focus its first usable entry.
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const box = el.getBoundingClientRect();
    const { left, top } = clampMenuPosition(x, y, box.width, box.height, window.innerWidth, window.innerHeight);
    el.style.left = `${left}px`;
    el.style.top = `${top}px`;
    el.style.visibility = 'visible';
    el.querySelectorAll<HTMLElement>('[role="menuitem"]')[Math.max(0, enabled.indexOf(true))]?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps -- position the menu once per opening
  }, [x, y]);

  // Return focus to whatever opened the menu when it goes away (unless the user clicked
  // elsewhere, which already moved the focus).
  useEffect(() => {
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    return () => {
      const active = document.activeElement;
      if (trigger?.isConnected && (!active || active === document.body)) trigger.focus();
    };
  }, []);

  // Close on anything that would leave the menu pointing at the wrong place.
  useEffect(() => {
    const close = () => onCloseRef.current();
    const onPointerDown = (e: PointerEvent) => {
      if (!ref.current?.contains(e.target as Node)) close();
    };
    window.addEventListener('pointerdown', onPointerDown, true);
    window.addEventListener('scroll', close, true);
    window.addEventListener('wheel', close, { capture: true, passive: true });
    window.addEventListener('resize', close);
    window.addEventListener('blur', close);
    return () => {
      window.removeEventListener('pointerdown', onPointerDown, true);
      window.removeEventListener('scroll', close, true);
      window.removeEventListener('wheel', close, true);
      window.removeEventListener('resize', close);
      window.removeEventListener('blur', close);
    };
  }, []);

  const items = () => [...(ref.current?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? [])];

  const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (NAV_KEYS.has(e.key)) {
      e.preventDefault();
      const list = items();
      const current = list.indexOf(document.activeElement as HTMLElement);
      list[nextEnabledIndex(enabled, current, e.key as MenuKey)]?.focus();
    } else if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    } else if (e.key === 'Tab') {
      e.preventDefault();
      onClose();
    }
  };

  return (
    <div
      ref={ref}
      className="context-menu"
      role="menu"
      aria-label={label}
      style={{ left: x, top: y, visibility: 'hidden' }}
      onKeyDown={onKeyDown}
      onContextMenu={(e) => e.preventDefault()}
    >
      {entries.map((entry, i) => {
        if (entry.type === 'separator') return <div key={`sep-${i}`} className="context-menu-sep" role="separator" />;
        return (
          <button
            key={entry.action}
            type="button"
            role="menuitem"
            tabIndex={-1}
            aria-disabled={entry.disabled || undefined}
            className={`context-menu-item${entry.danger ? ' danger' : ''}`}
            onClick={() => !entry.disabled && onSelect(entry.action)}
            onMouseEnter={(e) => !entry.disabled && e.currentTarget.focus()}
          >
            {entry.label}
          </button>
        );
      })}
    </div>
  );
}
