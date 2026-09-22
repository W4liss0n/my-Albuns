import { MenuItem, MenuSeparator } from "../ui/MenuItem";
import { projectCommandDescriptor } from "../application/projectCommandCatalog";
import type { SheetStructureAvailability } from "../application/sheetStructure";
import { ContextMenuSurface } from "../ui/ContextMenuSurface";


const sheetCommandLabels = {
  addAfter: projectCommandDescriptor("add-after").label,
  addBefore: projectCommandDescriptor("add-before").label,
  convertEdge: projectCommandDescriptor("convert-edge").label,
  deleteSheet: projectCommandDescriptor("delete-sheet").label,
  duplicateSheet: projectCommandDescriptor("duplicate-sheet").label,
} as const;



interface SheetContextMenuProps {
  availability: SheetStructureAvailability;
  position: { x: number; y: number };
  sheetNumber: number;
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

  return (
    <ContextMenuSurface label={`Ações da lâmina ${String(sheetNumber).padStart(2, "0")}`}
      position={position} onDismiss={onDismiss}>
        <MenuItem label={sheetCommandLabels.addBefore}
          disabled={!availability.canAddBefore}
          onClick={() => invoke(onAddBefore)} />
        <MenuItem label={sheetCommandLabels.addAfter}
          disabled={!availability.canAddAfter}
          onClick={() => invoke(onAddAfter)} />
        <MenuItem label={sheetCommandLabels.duplicateSheet}
          disabled={!availability.canDuplicate}
          onClick={() => invoke(onDuplicate)} />
        <MenuItem label={sheetCommandLabels.deleteSheet}
          disabled={!availability.canDelete}
          onClick={() => invoke(onDelete)} />
        <MenuSeparator />
        <MenuItem label={sheetCommandLabels.convertEdge}
          disabled={!availability.canConvertEdge}
          title={
            availability.canConvertEdge
              ? undefined
              : "Disponível somente para uma extremidade"
          }
          onClick={() => invoke(onConvertEdge)} />
    </ContextMenuSurface>
  );
}
