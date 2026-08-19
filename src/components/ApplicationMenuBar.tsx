import {
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";

export type ApplicationMenuCommand =
  | {
      availability: "implemented";
      disabled?: boolean;
      id: string;
      label: string;
      onSelect(): void;
      shortcut?: string;
      type: "command";
    }
  | {
      availability: "placeholder";
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

  useEffect(() => {
    if (openMenuId === null) return;

    const closeOnOutsidePointer = (event: PointerEvent) => {
      if (
        event.target instanceof Node &&
        !rootRef.current?.contains(event.target)
      ) {
        closeMenus();
      }
    };
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.key !== "Escape") return;
      event.preventDefault();
      if (openSubmenuIdRef.current !== null) {
        closeSubmenu(true);
      } else {
        closeMenus(true);
      }
    };
    const closeOnOutsideFocus = (event: FocusEvent) => {
      if (
        event.target instanceof Node &&
        !rootRef.current?.contains(event.target)
      ) {
        closeMenus();
      }
    };

    document.addEventListener("pointerdown", closeOnOutsidePointer);
    document.addEventListener("keydown", closeOnEscape);
    document.addEventListener("focusin", closeOnOutsideFocus);
    return () => {
      document.removeEventListener("pointerdown", closeOnOutsidePointer);
      document.removeEventListener("keydown", closeOnEscape);
      document.removeEventListener("focusin", closeOnOutsideFocus);
    };
  }, [openMenuId]);

  function focusFirstMenuItem(popupId: string) {
    queueMicrotask(() => {
      const popup = document.getElementById(popupId);
      if (!popup) return;
      const firstItem = enabledDirectMenuItems(popup)[0];
      (firstItem ?? popup).focus();
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
        focusRelativeMenuItem(event.currentTarget, currentItem, 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        focusRelativeMenuItem(event.currentTarget, currentItem, -1);
        break;
      case "Home":
        event.preventDefault();
        focusEdgeMenuItem(event.currentTarget, "first");
        break;
      case "End":
        event.preventDefault();
        focusEdgeMenuItem(event.currentTarget, "last");
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
        focusRelativeMenuItem(event.currentTarget, currentItem, 1);
        break;
      case "ArrowUp":
        event.preventDefault();
        focusRelativeMenuItem(event.currentTarget, currentItem, -1);
        break;
      case "Home":
        event.preventDefault();
        focusEdgeMenuItem(event.currentTarget, "first");
        break;
      case "End":
        event.preventDefault();
        focusEdgeMenuItem(event.currentTarget, "last");
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
      <button
        aria-label={item.label}
        data-placeholder-feature={placeholder ? item.feature : undefined}
        disabled={placeholder || item.disabled}
        key={item.id}
        role="menuitem"
        title={placeholder ? PLACEHOLDER_TITLE : undefined}
        type="button"
        onClick={() => {
          if (placeholder) return;
          closeMenus();
          item.onSelect();
        }}
        onPointerEnter={() => {
          if (!nested && openSubmenuIdRef.current !== null) closeSubmenu();
        }}
      >
        <span>{item.label}</span>
        {item.shortcut && (
          <span className="app-menu-shortcut">{item.shortcut}</span>
        )}
      </button>
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
                className="app-menu-popup"
                id={popupId}
                role="menu"
                tabIndex={-1}
                onKeyDown={(event) => handleMenuKeyDown(event, group.id)}
              >
                {group.items.map((item) => {
                  if (item.type === "separator") {
                    return (
                      <span
                        className="app-menu-separator"
                        key={item.id}
                        role="separator"
                      />
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
                      <button
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
                        role="menuitem"
                        type="button"
                        onClick={() =>
                          setOpenSubmenuId((current) =>
                            current === item.id ? null : item.id,
                          )
                        }
                        onPointerEnter={() => setOpenSubmenuId(item.id)}
                      >
                        <span>{item.label}</span>
                        <span aria-hidden="true" className="app-menu-cascade">
                          ›
                        </span>
                      </button>
                      {submenuOpen && (
                        <div
                          aria-label={item.label}
                          className="app-menu-popup app-menu-submenu-popup"
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

function enabledDirectMenuItems(container: HTMLElement) {
  return Array.from(container.children).flatMap((child) => {
    if (child instanceof HTMLButtonElement && !child.disabled) return [child];
    if (child instanceof HTMLDivElement) {
      const trigger = child.firstElementChild;
      if (trigger instanceof HTMLButtonElement && !trigger.disabled) {
        return [trigger];
      }
    }
    return [];
  });
}

function focusRelativeMenuItem(
  container: HTMLElement,
  currentItem: HTMLButtonElement | null,
  direction: -1 | 1,
) {
  const items = enabledDirectMenuItems(container);
  if (items.length === 0) {
    container.focus();
    return;
  }
  const currentIndex = currentItem ? items.indexOf(currentItem) : -1;
  const nextIndex =
    currentIndex < 0
      ? direction === 1
        ? 0
        : items.length - 1
      : (currentIndex + direction + items.length) % items.length;
  items[nextIndex]?.focus();
}

function focusEdgeMenuItem(container: HTMLElement, edge: "first" | "last") {
  const items = enabledDirectMenuItems(container);
  const item = edge === "first" ? items[0] : items[items.length - 1];
  (item ?? container).focus();
}
