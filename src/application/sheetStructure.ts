import type { SheetSnapshot } from "../domain/project";

export interface SheetStructureAvailability {
  canAddAfter: boolean;
  canAddBefore: boolean;
  canDelete: boolean;
}

export interface SheetReorderPlan {
  changed: boolean;
  order: string[];
  sourceIndex: number;
  targetIndex: number;
  valid: boolean;
}

export function sheetStructureAvailability(
  sheets: readonly SheetSnapshot[],
  sheetId: string,
): SheetStructureAvailability {
  const index = sheets.findIndex((sheet) => sheet.id === sheetId);
  if (index < 0) {
    return {
      canAddAfter: false,
      canAddBefore: false,
      canDelete: false,
    };
  }
  const sheet = sheets[index];
  return {
    canAddBefore: index > 0 || sheet.activeSides === "both",
    canAddAfter:
      index < sheets.length - 1 || sheet.activeSides === "both",
    canDelete: sheets.length > 2,
  };
}

export function planSheetReorder(
  sheets: readonly SheetSnapshot[],
  sheetId: string,
  targetIndex: number,
): SheetReorderPlan {
  const sourceIndex = sheets.findIndex((sheet) => sheet.id === sheetId);
  const order = sheets.map((sheet) => sheet.id);
  const inRange =
    Number.isInteger(targetIndex) &&
    targetIndex >= 0 &&
    targetIndex < sheets.length;
  if (sourceIndex < 0 || !inRange || sourceIndex === targetIndex) {
    return {
      changed: false,
      order,
      sourceIndex,
      targetIndex,
      valid: false,
    };
  }

  const candidate = [...sheets];
  const [moved] = candidate.splice(sourceIndex, 1);
  candidate.splice(targetIndex, 0, moved);
  const lastIndex = candidate.length - 1;
  const valid = candidate.every((sheet, index) => {
    if (index === 0) {
      return sheet.activeSides === "both" || sheet.activeSides === "right";
    }
    if (index === lastIndex) {
      return sheet.activeSides === "both" || sheet.activeSides === "left";
    }
    return sheet.activeSides === "both";
  });

  return {
    changed: true,
    order: candidate.map((sheet) => sheet.id),
    sourceIndex,
    targetIndex,
    valid,
  };
}
