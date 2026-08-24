import type { VisualPreviewGeometry } from "../ui/visualPreview";
import type { NewProjectDimensionsDraft } from "./application/newProjectDimensions";

export function createNewProjectPreviewGeometry(
  draft: NewProjectDimensionsDraft,
): VisualPreviewGeometry {
  return {
    bleedUm: draft.bleed.valueUm,
    heightUm: Math.max(1, draft.sheetHeight.valueUm),
    safetyUm: draft.safety.valueUm,
    widthUm: Math.max(1, draft.closedSheetWidth.valueUm * 2),
  };
}
