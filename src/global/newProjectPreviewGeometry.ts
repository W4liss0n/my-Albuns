import type { NewProjectDimensionsDraft } from "./application/newProjectDimensions";

export interface NewProjectPreviewGeometry {
  bleedUm: number;
  heightUm: number;
  safetyUm: number;
  widthUm: number;
}

export function createNewProjectPreviewGeometry(
  draft: NewProjectDimensionsDraft,
): NewProjectPreviewGeometry {
  return {
    bleedUm: draft.bleed.valueUm,
    heightUm: Math.max(1, draft.sheetHeight.valueUm),
    safetyUm: draft.safety.valueUm,
    widthUm: Math.max(1, draft.closedSheetWidth.valueUm * 2),
  };
}
