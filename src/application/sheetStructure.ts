import type {
  ProjectIntent,
  SheetSnapshot,
} from "../domain/project";

export type SheetStructureIntent = Extract<
  ProjectIntent,
  {
    kind: "addSheet" | "convertEdgeSheet" | "deleteSheet" | "reorderSheet";
  }
>;

export interface SheetStructureAvailability {
  canAddAfter: boolean;
  canAddBefore: boolean;
  canConvertEdge: boolean;
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
      canConvertEdge: false,
      canDelete: false,
    };
  }
  const sheet = sheets[index];
  return {
    canAddBefore: index > 0 || sheet.activeSides === "both",
    canAddAfter:
      index < sheets.length - 1 || sheet.activeSides === "both",
    canConvertEdge:
      (index === 0 || index === sheets.length - 1) && sheet.frames.length === 0,
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

export function isSheetStructureIntent(
  intent: ProjectIntent,
): intent is SheetStructureIntent {
  return (
    intent.kind === "addSheet" ||
    intent.kind === "convertEdgeSheet" ||
    intent.kind === "deleteSheet" ||
    intent.kind === "reorderSheet"
  );
}

/**
 * Revalidates a queued structural command against the authoritative Sheet
 * sequence. Sheet identity is never inferred from number or position: if the
 * original target disappeared, the command no longer has a valid materialized
 * form and is cancelled.
 */
export function materializeSheetStructureIntent(
  capturedSheets: readonly SheetSnapshot[],
  latestSheets: readonly SheetSnapshot[],
  intent: SheetStructureIntent,
): SheetStructureIntent | null {
  if (intent.kind === "reorderSheet") {
    const targetIndex = materializeSheetReorderTarget(
      capturedSheets,
      latestSheets,
      intent.sheetId,
      intent.targetIndex,
    );
    return targetIndex === null ? null : { ...intent, targetIndex };
  }

  const sheetId =
    intent.kind === "addSheet" ? intent.anchorSheetId : intent.sheetId;
  const availability = sheetStructureAvailability(latestSheets, sheetId);
  if (intent.kind === "addSheet") {
    const available =
      intent.position === "before"
        ? availability.canAddBefore
        : availability.canAddAfter;
    return available ? intent : null;
  }
  if (intent.kind === "deleteSheet") {
    return availability.canDelete ? intent : null;
  }
  return availability.canConvertEdge ? intent : null;
}

/**
 * Keeps a queued drag's semantic destination when an earlier mutation changes
 * the Sheet count. The captured neighbors define the user's destination; the
 * numeric index is recalculated only when the queued command actually runs.
 */
export function materializeSheetReorderTarget(
  capturedSheets: readonly SheetSnapshot[],
  latestSheets: readonly SheetSnapshot[],
  sheetId: string,
  targetIndex: number,
): number | null {
  const capturedPlan = planSheetReorder(
    capturedSheets,
    sheetId,
    targetIndex,
  );
  if (!capturedPlan.valid || !capturedPlan.changed) return null;
  if (!latestSheets.some((sheet) => sheet.id === sheetId)) return null;

  const destinationIndex = capturedPlan.order.indexOf(sheetId);
  const predecessorId = capturedPlan.order[destinationIndex - 1] ?? null;
  const successorId = capturedPlan.order[destinationIndex + 1] ?? null;
  const remainingOrder = latestSheets
    .map((sheet) => sheet.id)
    .filter((candidateId) => candidateId !== sheetId);
  const predecessorIndex = predecessorId
    ? remainingOrder.indexOf(predecessorId)
    : -1;
  const successorIndex = successorId
    ? remainingOrder.indexOf(successorId)
    : -1;

  let materializedTarget: number | null = null;
  if (predecessorIndex >= 0 && successorIndex >= 0) {
    if (predecessorIndex >= successorIndex) return null;
    materializedTarget = successorIndex;
  } else if (successorIndex >= 0) {
    materializedTarget = successorIndex;
  } else if (predecessorIndex >= 0) {
    materializedTarget = predecessorIndex + 1;
  } else if (destinationIndex === 0) {
    materializedTarget = 0;
  } else if (destinationIndex === capturedPlan.order.length - 1) {
    materializedTarget = remainingOrder.length;
  }
  if (materializedTarget === null) return null;

  const latestPlan = planSheetReorder(
    latestSheets,
    sheetId,
    materializedTarget,
  );
  return latestPlan.valid && latestPlan.changed
    ? materializedTarget
    : null;
}
