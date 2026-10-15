import { useEffect, useId, useLayoutEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

import { focusMenuItem } from "./menuNavigation";
import { MenuItem } from "./MenuItem";

const VIEWPORT_MARGIN_PX = 8;
const CLOSE_DELAY_MS = 200;

interface MenuSubmenuProps {
  label: string;
  disabled?: boolean;
  children: ReactNode;
}

/**
 * A cascading item for context menus. The nested menu stays inside the parent
 * menu's DOM, so the parent keeps owning outside-press and focus dismissal,
 * and it is placed with fixed coordinates so the parent's scroll box never
 * clips it.
 */
export function MenuSubmenu({ label, disabled = false, children }: MenuSubmenuProps) {
  const [open, setOpen] = useState(false);
  const [position, setPosition] = useState<{ left: number; top: number } | null>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const menuRef = useRef<HTMLDivElement>(null);
  const closeTimer = useRef<number | undefined>(undefined);
  const focusOnOpen = useRef(false);
  const menuId = useId();

  useEffect(() => () => window.clearTimeout(closeTimer.current), []);
  useEffect(() => { if (disabled) setOpen(false); }, [disabled]);

  useLayoutEffect(() => {
    if (!open) { setPosition(null); return; }
    const trigger = triggerRef.current?.getBoundingClientRect();
    const menu = menuRef.current?.getBoundingClientRect();
    if (!trigger || !menu) return;
    // Overlap the parent by its padding so the first items line up.
    let left = trigger.right + 4;
    if (left + menu.width > window.innerWidth - VIEWPORT_MARGIN_PX) {
      left = Math.max(VIEWPORT_MARGIN_PX, trigger.left - 4 - menu.width);
    }
    const top = Math.max(VIEWPORT_MARGIN_PX,
      Math.min(trigger.top - 5, window.innerHeight - menu.height - VIEWPORT_MARGIN_PX));
    setPosition({ left, top });
  }, [open]);

  // Chromium does not focus inside the hidden menu that is being measured.
  useLayoutEffect(() => {
    if (!position || !focusOnOpen.current || !menuRef.current) return;
    focusOnOpen.current = false;
    focusMenuItem(menuRef.current, "first", { fallbackToContainer: false, preventScroll: true });
  }, [position]);

  function show(focus: boolean) {
    if (disabled) return;
    window.clearTimeout(closeTimer.current);
    if (open) {
      if (focus && menuRef.current) focusMenuItem(menuRef.current, "first", { fallbackToContainer: false });
      return;
    }
    focusOnOpen.current = focus;
    setOpen(true);
  }

  function hide(restoreFocus: boolean) {
    window.clearTimeout(closeTimer.current);
    setOpen(false);
    if (restoreFocus) triggerRef.current?.focus();
  }

  function handleMenuKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      focusMenuItem(event.currentTarget, event.key === "ArrowDown" ? "next" : "previous", {
        current: event.target,
        fallbackToContainer: false,
      });
    } else if (event.key === "ArrowLeft" || event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      hide(true);
    }
  }

  return (
    <div
      className="ui-menu-submenu"
      onPointerEnter={() => show(false)}
      onPointerLeave={() => {
        window.clearTimeout(closeTimer.current);
        closeTimer.current = window.setTimeout(() => setOpen(false), CLOSE_DELAY_MS);
      }}
    >
      <MenuItem
        ref={triggerRef}
        aria-controls={open ? menuId : undefined}
        aria-expanded={open}
        aria-haspopup="menu"
        disabled={disabled}
        label={label}
        trailing={<span aria-hidden="true" className="ui-menu-item__cascade">›</span>}
        onClick={() => show(true)}
        onKeyDown={(event) => {
          if (event.key === "ArrowRight" || event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            event.stopPropagation();
            show(true);
          }
        }}
      />
      {open && (
        <div
          ref={menuRef}
          aria-label={label}
          className="ui-floating-surface ui-context-menu ui-context-submenu"
          id={menuId}
          role="menu"
          style={position ?? { left: 0, top: 0, visibility: "hidden" }}
          tabIndex={-1}
          onKeyDown={handleMenuKeyDown}
        >
          {children}
        </div>
      )}
    </div>
  );
}
