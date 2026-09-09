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
  function invoke(action: () => void) {
    action();
    onDismiss();
  }

  return (
    <ContextMenuSurface label={`Ações da Lâmina ${String(sheetNumber).padStart(2, "0")}`}
      position={position} onDismiss={onDismiss}>
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
        <span className="ui-context-menu__separator" role="separator" />
        <button
          disabled={!availability.canConvertEdge}
          role="menuitem"
          title={
            availability.canConvertEdge
              ? undefined
              : "Disponível somente para uma extremidade"
          }
          type="button"
          onClick={() => invoke(onConvertEdge)}
        >
          {sheetCommandLabels.convertEdge}
        </button>
    </ContextMenuSurface>
  );
}
