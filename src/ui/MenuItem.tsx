import { createElement, Fragment, forwardRef, type ComponentPropsWithoutRef, type ReactNode } from "react";
import "./MenuItem.css";

interface MenuItemProps extends Omit<ComponentPropsWithoutRef<"button">, "children"> {
  label: string;
  shortcut?: string;
  checked?: boolean;
  trailing?: ReactNode;
  compact?: boolean;
}

export const MenuItem = forwardRef<HTMLButtonElement, MenuItemProps>(
  function MenuItem({ label, shortcut, checked, trailing, compact = false, className, ...props }, ref) {
    return (
      <button
        aria-label={label}
        aria-checked={checked}
        role={checked === undefined ? "menuitem" : "menuitemcheckbox"}
        type="button"
        {...props}
        className={`ui-menu-item${compact ? " ui-menu-item--compact" : ""}${className ? ` ${className}` : ""}`}
        ref={ref}
      >
        <span>
          {checked !== undefined && <span aria-hidden="true" className="ui-menu-item__checkmark">{checked ? "✓" : ""}</span>}
          {label}
        </span>
        {shortcut && <kbd aria-hidden="true" className="ui-menu-item__shortcut">{shortcut}</kbd>}
        {trailing}
      </button>
    );
  },
);

export function MenuSeparator() {
  return <span className="ui-menu-separator" role="separator" />;
}

/**
 * Renders groups of related menu items with a separator between them.
 * Hidden items (`false`, `null`, `undefined`) are dropped, and a group left
 * empty adds no separator, so conditional items never leave a stray divider.
 */
export function MenuGroups({ groups }: { groups: readonly (readonly ReactNode[])[] }) {
  const visible = groups
    .map((group) => group.filter((item) => item !== false && item !== null && item !== undefined))
    .filter((group) => group.length > 0);
  return visible.map((group, index) => createElement(Fragment, { key: index },
    ...(index > 0 ? [<MenuSeparator />] : []), ...group));
}
