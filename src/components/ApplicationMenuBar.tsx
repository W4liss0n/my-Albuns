import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { useDismissableSurface } from "../ui/useDismissableSurface";
import { focusMenuItem } from "../ui/menuNavigation";
import { MenuItem, MenuSeparator } from "../ui/MenuItem";

import "./ApplicationMenuBar.css";

export type ApplicationMenuCommand =
  | {
      availability: "implemented";
      checked?: boolean;
      context?: string;
      disabled?: boolean;
      title?: string;
      id: string;
      label: string;
      onSelect(): void;
      shortcut?: string;
      type: "command";
    }
  | {
      availability: "placeholder";
      context?: string;
      feature: string;
      id: string;
      label: string;
      shortcut?: string;
      type: "command";
    };

export interface ApplicationMenuSeparator {
  id: string;
  type: "separator";
}

export interface ApplicationMenuSubmenu {
  id: string;
  items: readonly ApplicationMenuCommand[];
  label: string;
  type: "submenu";
}

export type ApplicationMenuItem =
  | ApplicationMenuCommand
  | ApplicationMenuSeparator
  | ApplicationMenuSubmenu;

export interface ApplicationMenuGroup {
  id: string;
  items: readonly ApplicationMenuItem[];
  label: string;
}

interface ApplicationMenuBarProps {
  disabled?: boolean;
  groups: readonly ApplicationMenuGroup[];
}

const PLACEHOLDER_TITLE = "Ainda não disponível nesta versão";

export function ApplicationMenuBar({
  disabled = false,
  groups,
}: ApplicationMenuBarProps) {
  const [openMenuId, setOpenMenuId] = useState<string | null>(null);
  const [openSubmenuId, setOpenSubmenuId] = useState<string | null>(null);
  const rootRef = useRef<HTMLElement>(null);
  const openMenuIdRef = useRef(openMenuId);
  const openSubmenuIdRef = useRef(openSubmenuId);
  const topMenuButtonRefs = useRef(new Map<string, HTMLButtonElement>());
  const submenuButtonRefs = useRef(new Map<string, HTMLButtonElement>());
  openMenuIdRef.current = openMenuId;
  openSubmenuIdRef.current = openSubmenuId;

  useEffect(() => {
    if (disabled) closeMenus();
  }, [disabled]);

  useEffect(() => {
    if (openMenuId !== null) {
      focusFirstMenuItem(`application-menu-${openMenuId}`);
    }
  }, [openMenuId]);

  useEffect(() => {
    if (openSubmenuId !== null) {
      focusFirstMenuItem(`application-submenu-${openSubmenuId}`);
    }
  }, [openSubmenuId]);

  useDismissableSurface({
    enabled: openMenuId !== null,
    includeFocusOutside: true,
    rootRef,
    onDismiss: ({ reason, event }) => {
      if (reason === "pointerOutside" || reason === "focusOutside") {
        closeMenus();
        return;
      }
      if (event.defaultPrevented || event.key !== "Escape") return;
      event.preventDefault();
      if (openSubmenuIdRef.current !== null) {
        closeSubmenu(true);
      } else {
        closeMenus(true);
      }
    },
  });

  function focusFirstMenuItem(popupId: string) {
    queueMicrotask(() => {
      const popup = document.getElementById(popupId);
      if (!popup) return;
      focusMenuItem(popup, "first");
    });
  }

  function closeMenus(restoreFocus = false) {
    const menuId = openMenuIdRef.current;
    setOpenSubmenuId(null);
    setOpenMenuId(null);
    if (restoreFocus && menuId !== null) {
      queueMicrotask(() => topMenuButtonRefs.current.get(menuId)?.focus());
    }
  }

  function closeSubmenu(restoreFocus = false) {
    const submenuId = openSubmenuIdRef.current;
    setOpenSubmenuId(null);
    if (restoreFocus && submenuId !== null) {
      queueMicrotask(() => submenuButtonRefs.current.get(submenuId)?.focus());
    }
  }

  function openMenu(menuId: string) {
    setOpenSubmenuId(null);
    setOpenMenuId(menuId);
    if (openMenuIdRef.current === menuId) {
      focusFirstMenuItem(`application-menu-${menuId}`);
    }
  }

  function adjacentMenuId(menuId: string, direction: -1 | 1) {
    const index = groups.findIndex((group) => group.id === menuId);
    if (index < 0 || groups.length === 0) return null;
    return groups[(index + direction + groups.length) % groups.length]?.id ?? null;
  }

  function focusAdjacentTopMenu(menuId: string, direction: -1 | 1) {
    const adjacentId = adjacentMenuId(menuId, direction);
    if (adjacentId === null) return;
    if (openMenuIdRef.current === null) {
      topMenuButtonRefs.current.get(adjacentId)?.focus();
    } else {
      openMenu(adjacentId);
    }
  }

  function handleTopMenuKeyDown(
    event: ReactKeyboardEvent<HTMLButtonElement>,
    menuId: string,
  ) {
    switch (event.key) {
      case "ArrowDown":
      case "Enter":
      case " ":
        event.preventDefault();
        openMenu(menuId);
        break;
      case "ArrowLeft":
        event.preventDefault();
        focusAdjacentTopMenu(menuId, -1);
        break;
      case "ArrowRight":
        event.preventDefault();
        focusAdjacentTopMenu(menuId, 1);
        break;
      case "Escape":
        if (openMenuIdRef.current !== null) {
          event.preventDefault();
          closeMenus(true);
        }
        break;
      default:
        break;
    }
  }

  function handleMenuKeyDown(
    event: ReactKeyboardEvent<HTMLDivElement>,
    menuId: string,
  ) {
    const currentItem =
      event.target instanceof HTMLButtonElement ? event.target : null;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "next", { current: currentItem });
        break;
      case "ArrowUp":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "previous", { current: currentItem });
        break;
      case "Home":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "first");
        break;
      case "End":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "last");
        break;
      case "ArrowRight":
        event.preventDefault();
        if (currentItem?.dataset.submenuTrigger) {
          setOpenSubmenuId(currentItem.dataset.submenuTrigger);
        } else {
          focusAdjacentTopMenu(menuId, 1);
        }
        break;
      case "ArrowLeft":
        event.preventDefault();
        focusAdjacentTopMenu(menuId, -1);
        break;
      case "Escape":
        event.preventDefault();
        closeMenus(true);
        break;
      default:
        break;
    }
  }

  function handleSubmenuKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    event.stopPropagation();
    const currentItem =
      event.target instanceof HTMLButtonElement ? event.target : null;
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "next", { current: currentItem });
        break;
      case "ArrowUp":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "previous", { current: currentItem });
        break;
      case "Home":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "first");
        break;
      case "End":
        event.preventDefault();
        focusMenuItem(event.currentTarget, "last");
        break;
      case "ArrowLeft":
      case "Escape":
        event.preventDefault();
        closeSubmenu(true);
        break;
      default:
        break;
    }
  }

  function renderCommand(item: ApplicationMenuCommand, nested = false) {
    const placeholder = item.availability === "placeholder";
    return (
      <MenuItem
        label={item.label}
        shortcut={item.shortcut}
        checked={!placeholder ? item.checked : undefined}
        data-placeholder-feature={placeholder ? item.feature : undefined}
        disabled={placeholder || item.disabled}
        key={item.id}
        title={placeholder ? PLACEHOLDER_TITLE : item.title}
        onClick={() => {
          if (placeholder) return;
          closeMenus();
          item.onSelect();
        }}
        onPointerEnter={() => {
          if (!nested && openSubmenuIdRef.current !== null) closeSubmenu();
        }}
      />
    );
  }

  return (
    <nav
      ref={rootRef}
      aria-label="Menu principal"
      className="app-menu"
      role="menubar"
    >
      {groups.map((group) => {
        const open = openMenuId === group.id;
        const popupId = `application-menu-${group.id}`;
        return (
          <div className="app-menu-entry" key={group.id} role="none">
            <button
              ref={(node) => {
                if (node) topMenuButtonRefs.current.set(group.id, node);
                else topMenuButtonRefs.current.delete(group.id);
              }}
              aria-controls={popupId}
              aria-expanded={open}
              aria-haspopup="menu"
              disabled={disabled}
              role="menuitem"
              type="button"
              onClick={() => (open ? closeMenus() : openMenu(group.id))}
              onKeyDown={(event) => handleTopMenuKeyDown(event, group.id)}
              onPointerEnter={() => {
                if (openMenuIdRef.current !== null) openMenu(group.id);
              }}
            >
              {group.label}
            </button>
            {open && (
              <div
                aria-label={group.label}
                className="ui-floating-surface app-menu-popup"
                id={popupId}
                role="menu"
                tabIndex={-1}
                onKeyDown={(event) => handleMenuKeyDown(event, group.id)}
              >
                {group.items.map((item) => {
                  if (item.type === "separator") {
                    return (
                      <MenuSeparator key={item.id} />
                    );
                  }
                  if (item.type === "command") return renderCommand(item);

                  const submenuOpen = openSubmenuId === item.id;
                  const submenuId = `application-submenu-${item.id}`;
                  return (
                    <div
                      className="app-menu-submenu"
                      key={item.id}
                      role="none"
                    >
                      <MenuItem
                        ref={(node) => {
                          if (node) {
                            submenuButtonRefs.current.set(item.id, node);
                          } else {
                            submenuButtonRefs.current.delete(item.id);
                          }
                        }}
                        aria-controls={submenuId}
                        aria-expanded={submenuOpen}
                        aria-haspopup="menu"
                        data-submenu-trigger={item.id}
                        label={item.label}
                        trailing={<span aria-hidden="true" className="app-menu-cascade">›</span>}
                        onClick={() =>
                          setOpenSubmenuId(item.id)
                        }
                        onPointerEnter={() => setOpenSubmenuId(item.id)}
                      />
                      {submenuOpen && (
                        <div
                          aria-label={item.label}
                          className="ui-floating-surface app-menu-popup app-menu-submenu-popup"
                          id={submenuId}
                          role="menu"
                          tabIndex={-1}
                          onKeyDown={handleSubmenuKeyDown}
                        >
                          {item.items.map((command) =>
                            renderCommand(command, true),
                          )}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        );
      })}
    </nav>
  );
}
