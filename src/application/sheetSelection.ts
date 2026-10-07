import { projectCommandDescriptor } from "./projectCommandCatalog";

export interface SheetSelectionModifiers {
  /** Ctrl (Cmd on macOS): add or remove one Sheet. */
  readonly toggle: boolean;
  /** Shift: select from the anchor to the clicked Sheet. */
  readonly range: boolean;
}

export interface SheetSelection {
  readonly selectedSheetIds: readonly string[];
  readonly focusedSheetId: string | null;
  readonly anchorSheetId: string | null;
}

export const PLAIN_SHEET_SELECTION: SheetSelectionModifiers = {
  toggle: false,
  range: false,
};

export function sheetSelectionModifiers(event: {
  readonly ctrlKey: boolean;
  readonly metaKey: boolean;
  readonly shiftKey: boolean;
}): SheetSelectionModifiers {
  return { toggle: event.ctrlKey || event.metaKey, range: event.shiftKey };
}

/**
 * The focused Sheet is always part of the selection. A list that does not
 * contain it is stale (the focus moved through the Bar, the Canvas or a
 * navigation), so the selection is the focused Sheet alone.
 */
export function effectiveSelectedSheetIds(
  selectedSheetIds: readonly string[],
  focusedSheetId: string | null,
): readonly string[] {
  if (focusedSheetId === null) return selectedSheetIds;
  return selectedSheetIds.includes(focusedSheetId)
    ? selectedSheetIds
    : [focusedSheetId];
}

/**
 * Explorer-like click selection over the confirmed Sheet order. The result is
 * never empty: Ctrl on the only selected Sheet keeps it, because the editor
 * always has a focused Sheet.
 */
export function nextSheetSelection(
  current: SheetSelection,
  sheetId: string,
  modifiers: SheetSelectionModifiers,
  orderedSheetIds: readonly string[],
): SheetSelection {
  const targetIndex = orderedSheetIds.indexOf(sheetId);
  if (targetIndex < 0) return current;
  const selected = new Set(
    effectiveSelectedSheetIds(
      current.selectedSheetIds,
      current.focusedSheetId,
    ).filter((id) => orderedSheetIds.includes(id)),
  );
  const inOrder = (ids: ReadonlySet<string>) =>
    orderedSheetIds.filter((id) => ids.has(id));

  const anchorIndex = current.anchorSheetId === null
    ? -1
    : orderedSheetIds.indexOf(current.anchorSheetId);
  if (modifiers.range && anchorIndex >= 0) {
    const range = orderedSheetIds.slice(
      Math.min(anchorIndex, targetIndex),
      Math.max(anchorIndex, targetIndex) + 1,
    );
    if (modifiers.toggle) range.forEach((id) => selected.add(id));
    return {
      selectedSheetIds: modifiers.toggle ? inOrder(selected) : range,
      focusedSheetId: sheetId,
      anchorSheetId: current.anchorSheetId,
    };
  }

  if (modifiers.toggle) {
    if (!selected.has(sheetId)) {
      selected.add(sheetId);
      return {
        selectedSheetIds: inOrder(selected),
        focusedSheetId: sheetId,
        anchorSheetId: sheetId,
      };
    }
    if (selected.size > 1) {
      selected.delete(sheetId);
      const remaining = inOrder(selected);
      return {
        selectedSheetIds: remaining,
        focusedSheetId:
          current.focusedSheetId !== null &&
          selected.has(current.focusedSheetId)
            ? current.focusedSheetId
            : nearestSheetId(remaining, orderedSheetIds, targetIndex),
        anchorSheetId: sheetId,
      };
    }
  }

  return {
    selectedSheetIds: [sheetId],
    focusedSheetId: sheetId,
    anchorSheetId: sheetId,
  };
}

function nearestSheetId(
  candidates: readonly string[],
  orderedSheetIds: readonly string[],
  index: number,
): string | null {
  let nearest: string | null = null;
  let nearestDistance = Number.POSITIVE_INFINITY;
  for (const id of candidates) {
    const distance = Math.abs(orderedSheetIds.indexOf(id) - index);
    if (distance < nearestDistance) {
      nearest = id;
      nearestDistance = distance;
    }
  }
  return nearest;
}

/** Menu label of Delete or Duplicate for the number of Sheets it will change. */
export function sheetSelectionCommandLabel(
  command: "delete-sheet" | "duplicate-sheet",
  sheetCount: number,
): string {
  if (sheetCount <= 1) return projectCommandDescriptor(command).label;
  return command === "delete-sheet"
    ? `Excluir ${sheetCount} lâminas`
    : `Duplicar ${sheetCount} lâminas`;
}

/**
 * Sheets that Delete and Duplicate change: a selection made in the Grade while
 * the Grade shows it, otherwise the centered (or edited) Sheet.
 */
export function sheetCommandTargetIds({
  gridShowsSelection,
  selectedSheetIds,
  source,
  implicitSheetId,
}: {
  readonly gridShowsSelection: boolean;
  readonly selectedSheetIds: readonly string[];
  readonly source: "grid" | "focus";
  readonly implicitSheetId: string | null;
}): readonly string[] {
  if (gridShowsSelection && source === "grid" && selectedSheetIds.length > 0) {
    return selectedSheetIds;
  }
  return implicitSheetId ? [implicitSheetId] : [];
}

/** A Sheet menu opened on one of several targets acts on all of them. */
export function sheetContextMenuTargetIds(
  clickedSheetId: string,
  commandTargetIds: readonly string[],
): readonly string[] {
  return commandTargetIds.length > 1 && commandTargetIds.includes(clickedSheetId)
    ? commandTargetIds
    : [clickedSheetId];
}
