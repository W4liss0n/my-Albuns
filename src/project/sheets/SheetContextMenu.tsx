import { MenuItem, MenuSeparator } from "../../ui/MenuItem";
import { projectCommandDescriptor } from "../../application/projectCommandCatalog";
import type { SheetStructureAvailability } from "../../application/sheetStructure";
import { ContextMenuSurface } from "../../ui/ContextMenuSurface";
import { sheetSelectionCommandLabel } from "../../application/sheetSelection";


const sheetCommandLabels = {
  addAfter: projectCommandDescriptor("add-after").label,
  addBefore: projectCommandDescriptor("add-before").label,
  convertEdge: projectCommandDescriptor("convert-edge").label,
} as const;

const SINGLE_SHEET_ONLY = "Disponível somente para uma lâmina";



interface SheetContextMenuProps {
  availability: SheetStructureAvailability;
  position: { x: number; y: number };
  sheetNumber: number;
  /** Sheets the menu acts on: the clicked one, or the selection it belongs to. */
  sheetCount?: number;
  onAddAfter(): void;
  onAddBefore(): void;
  onConvertEdge(): void;
  onDelete(): void;
  onDuplicate(): void;
  onDismiss(): void;
}

export function SheetContextMenu({
  availability,
  position,
  sheetNumber,
  sheetCount = 1,
  onAddAfter,
  onAddBefore,
  onConvertEdge,
  onDelete,
  onDuplicate,
  onDismiss,
}: SheetContextMenuProps) {
  function invoke(action: () => void) {
    action();
    onDismiss();
  }

  // Adding and converting need one anchor Sheet; with several Sheets only
  // Duplicate and Delete apply, to all of them.
  const several = sheetCount > 1;
  return (
    <ContextMenuSurface
      label={several
        ? `Ações das ${sheetCount} lâminas`
        : `Ações da lâmina ${String(sheetNumber).padStart(2, "0")}`}
      position={position} onDismiss={onDismiss}>
        <MenuItem label={sheetCommandLabels.addBefore}
          disabled={several || !availability.canAddBefore}
          title={several ? SINGLE_SHEET_ONLY : undefined}
          onClick={() => invoke(onAddBefore)} />
        <MenuItem label={sheetCommandLabels.addAfter}
          disabled={several || !availability.canAddAfter}
          title={several ? SINGLE_SHEET_ONLY : undefined}
          onClick={() => invoke(onAddAfter)} />
        <MenuItem label={sheetSelectionCommandLabel("duplicate-sheet", sheetCount)}
          disabled={!availability.canDuplicate}
          onClick={() => invoke(onDuplicate)} />
        <MenuItem label={sheetSelectionCommandLabel("delete-sheet", sheetCount)}
          disabled={!availability.canDelete}
          onClick={() => invoke(onDelete)} />
        <MenuSeparator />
        <MenuItem label={sheetCommandLabels.convertEdge}
          disabled={several || !availability.canConvertEdge}
          title={
            several
              ? SINGLE_SHEET_ONLY
              : availability.canConvertEdge
                ? undefined
                : "Disponível somente para uma extremidade"
          }
          onClick={() => invoke(onConvertEdge)} />
    </ContextMenuSurface>
  );
}
