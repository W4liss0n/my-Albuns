import { useId, useRef, useState, type KeyboardEvent } from "react";
import { ChevronDown } from "lucide-react";
import { AppIcon } from "../ui";
import { useDismissableSurface } from "../ui/useDismissableSurface";

const GRID_COLUMNS = 6;

interface LayoutPositionCountPickerProps {
  value: number;
  counts: readonly number[];
  disabled: boolean;
  onChange(count: number): void;
}

/**
 * Frame count inside the panel title ("Layouts com [4] quadros"). Closed, the
 * arrows step the value like a native select; open, every count shows at once
 * in a compact grid instead of a long list.
 */
export function LayoutPositionCountPicker({ value, counts, disabled, onChange }: LayoutPositionCountPickerProps) {
  const listId = useId();
  const rootRef = useRef<HTMLSpanElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const [anchor, setAnchor] = useState<{ left: number; top: number } | null>(null);
  const open = anchor !== null && !disabled;
  const close = (restoreFocus: boolean) => {
    setAnchor(null);
    if (restoreFocus) triggerRef.current?.focus();
  };
  // The grid lives inside the Layout panel, so a press on it never counts as
  // outside the panel; only presses outside the picker close the grid.
  useDismissableSurface({ enabled: open, rootRef, onDismiss({ reason }) {
    if (reason === "pointerOutside") close(false);
  } });
  const choose = (count: number) => {
    close(true);
    if (count !== value) onChange(count);
  };
  const step = (offset: number) => {
    const next = counts[counts.indexOf(value) + offset];
    if (next !== undefined) onChange(next);
  };
  const openGrid = () => {
    const bounds = triggerRef.current?.getBoundingClientRect();
    if (bounds) setAnchor({ left: bounds.left - 8, top: bounds.bottom + 6 });
  };
  const onTriggerKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
    if (event.altKey && event.key === "ArrowDown") { event.preventDefault(); openGrid(); return; }
    const offset = event.key === "ArrowDown" || event.key === "ArrowRight" ? 1
      : event.key === "ArrowUp" || event.key === "ArrowLeft" ? -1 : 0;
    if (offset !== 0 && !open) { event.preventDefault(); step(offset); }
  };
  const onGridKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const options = [...event.currentTarget.querySelectorAll<HTMLButtonElement>('[role="option"]')];
    const index = options.indexOf(document.activeElement as HTMLButtonElement);
    const offset = ({ ArrowRight: 1, ArrowLeft: -1, ArrowDown: GRID_COLUMNS, ArrowUp: -GRID_COLUMNS } as Record<string, number>)[event.key];
    if (event.key === "Escape" || event.key === "Tab") {
      if (event.key === "Escape") { event.preventDefault(); event.stopPropagation(); }
      close(event.key === "Escape");
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      options[event.key === "Home" ? 0 : options.length - 1]?.focus();
    } else if (offset !== undefined && index >= 0) {
      event.preventDefault();
      options[Math.min(options.length - 1, Math.max(0, index + offset))]?.focus();
    }
  };
  return (
    <span className="layout-panel__count" ref={rootRef}>
      <button aria-controls={open ? listId : undefined} aria-expanded={open} aria-haspopup="listbox"
        aria-label="Quantidade de quadros" className="layout-panel__count-trigger" data-minimum={counts[0]}
        disabled={disabled} ref={triggerRef} role="combobox" type="button"
        onClick={() => (open ? close(false) : openGrid())} onKeyDown={onTriggerKeyDown}>
        <span>{value}</span>
        <AppIcon icon={ChevronDown} size={12} />
      </button>
      {open && <div aria-label="Quantidade de quadros" className="ui-floating-surface layout-panel__count-grid"
        id={listId} role="listbox" style={{ left: anchor.left, top: anchor.top }} onKeyDown={onGridKeyDown}>
        {counts.map((count) => <button aria-selected={count === value} autoFocus={count === value}
          className="layout-panel__count-option" key={count} role="option" tabIndex={count === value ? 0 : -1}
          type="button" onClick={() => choose(count)}>{count}</button>)}
      </div>}
    </span>
  );
}
