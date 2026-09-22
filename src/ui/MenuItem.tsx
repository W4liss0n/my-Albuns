import { forwardRef, type ComponentPropsWithoutRef, type ReactNode } from "react";
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
