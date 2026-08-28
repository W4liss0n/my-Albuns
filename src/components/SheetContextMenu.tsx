import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";

import { projectCommandDescriptor } from "../application/projectCommandCatalog";
import type { SheetStructureAvailability } from "../application/sheetStructure";
import { useDismissableSurface } from "../ui/useDismissableSurface";
import "./SheetContextMenu.css";

const sheetCommandLabels = {
  addAfter: projectCommandDescriptor("add-after").label,
  addBefore: projectCommandDescriptor("add-before").label,
  convertEdge: projectCommandDescriptor("convert-edge").label,
  deleteSheet: projectCommandDescriptor("delete-sheet").label,
  duplicateSheet: projectCommandDescriptor("duplicate-sheet").label,
} as const;

const VIEWPORT_MARGIN_PX = 8;

interface SheetContextMenuProps {
  availability: SheetStructureAvailability;
  position: { x: number; y: number };
  sheetNumber: number;
  onAddAfter(): void;
  onAddBefore(): void;
  onConvertEdge(): void;
  onDelete(): void;
  onDismiss(): void;
}

export function SheetContextMenu({
  availability,
  position,
  sheetNumber,
  onAddAfter,
  onAddBefore,
  onConvertEdge,
  onDelete,
  onDismiss,
}: SheetContextMenuProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const [visiblePosition, setVisiblePosition] = useState(position);

  useLayoutEffect(() => {
    const keepInsideViewport = () => {
      const root = rootRef.current;
      if (!root) return;
      const bounds = root.getBoundingClientRect();
      const maximumX = Math.max(
        VIEWPORT_MARGIN_PX,
        window.innerWidth - bounds.width - VIEWPORT_MARGIN_PX,
      );
      const maximumY = Math.max(
        VIEWPORT_MARGIN_PX,
        window.innerHeight - bounds.height - VIEWPORT_MARGIN_PX,
      );
      const next = {
        x: Math.min(Math.max(position.x, VIEWPORT_MARGIN_PX), maximumX),
        y: Math.min(Math.max(position.y, VIEWPORT_MARGIN_PX), maximumY),
      };
      setVisiblePosition((current) =>
        current.x === next.x && current.y === next.y ? current : next,
      );
    };

    keepInsideViewport();
    window.addEventListener("resize", keepInsideViewport);
    return () => window.removeEventListener("resize", keepInsideViewport);
  }, [position.x, position.y]);

  useDismissableSurface({
    enabled: true,
    includeFocusOutside: true,
    rootRef,
    onDismiss: ({ reason, event }) => {
      if (reason === "escape") event.preventDefault();
      onDismiss();
    },
  });

  useEffect(() => {
    queueMicrotask(() => {
      rootRef.current
        ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
        ?.focus();
    });
  }, []);

  function invoke(action: () => void) {
    action();
    onDismiss();
  }

  function handleKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const items = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ),
    );
    if (items.length === 0) return;
    const current =
      event.target instanceof HTMLButtonElement
        ? items.indexOf(event.target)
        : -1;
    const delta = event.key === "ArrowDown" ? 1 : -1;
    const next =
      current < 0
        ? delta > 0
          ? 0
          : items.length - 1
        : (current + delta + items.length) % items.length;
    items[next]?.focus();
  }

  return (
    <div
      ref={rootRef}
      aria-label={`Ações da Lâmina ${String(sheetNumber).padStart(2, "0")}`}
      className="ui-floating-surface sheet-context-menu"
      role="menu"
      style={{ left: visiblePosition.x, top: visiblePosition.y }}
      tabIndex={-1}
      onContextMenu={(event) => event.preventDefault()}
      onKeyDown={handleKeyDown}
    >
      <button
        disabled={!availability.canAddBefore}
        role="menuitem"
        type="button"
        onClick={() => invoke(onAddBefore)}
      >
        {sheetCommandLabels.addBefore}
      </button>
      <button
        disabled={!availability.canAddAfter}
        role="menuitem"
        type="button"
        onClick={() => invoke(onAddAfter)}
      >
        {sheetCommandLabels.addAfter}
      </button>
      <button
        disabled
        role="menuitem"
        title="Ainda não disponível nesta versão"
        type="button"
      >
        {sheetCommandLabels.duplicateSheet}
      </button>
      <button
        disabled={!availability.canDelete}
        role="menuitem"
        type="button"
        onClick={() => invoke(onDelete)}
      >
        {sheetCommandLabels.deleteSheet}
      </button>
      <span className="sheet-context-menu__separator" role="separator" />
      <button
        disabled={!availability.canConvertEdge}
        role="menuitem"
        title={
          availability.canConvertEdge
            ? undefined
            : "Disponível somente para uma extremidade vazia"
        }
        type="button"
        onClick={() => invoke(onConvertEdge)}
      >
        {sheetCommandLabels.convertEdge}
      </button>
    </div>
  );
}
