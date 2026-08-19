import type { ComposedSheet } from "../domain/project";
import { createSheetGuideGeometry } from "../ui/sheetGuideGeometry";
import {
  createContinuousCanvasLayout,
  MICROMETER_TO_CANVAS_PIXEL,
  type CanvasSheetPresentation,
} from "./canvasGeometry";

export interface CanvasBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface CanvasSheetViewGeometry {
  activeBounds: CanvasBounds;
  inactiveSideBounds: CanvasBounds | null;
  visibleOuterBounds: CanvasBounds;
}

export function createCanvasSheetViewGeometry(
  sheet: ComposedSheet,
  presentation: CanvasSheetPresentation,
  bleedUm: number | undefined,
  cropsBleed: boolean,
): CanvasSheetViewGeometry {
  const fullHeight = sheet.heightUm * MICROMETER_TO_CANVAS_PIXEL;
  const bleedInset = cropsBleed
    ? createSheetGuideGeometry({
        bleedUm: bleedUm ?? 0,
        heightUm: sheet.heightUm,
        safetyUm: 0,
      }).bleedInsetUm * MICROMETER_TO_CANVAS_PIXEL
    : 0;
  const visibleOuterBounds: CanvasBounds = {
    x: bleedInset,
    y: bleedInset,
    width: Math.max(
      0,
      presentation.visualWidthPx - bleedInset * 2,
    ),
    height: Math.max(0, fullHeight - bleedInset * 2),
  };
  const activeBounds = intersectBounds(
    {
      x: presentation.activeOffsetXPx,
      y: 0,
      width: presentation.activeWidthPx,
      height: fullHeight,
    },
    visibleOuterBounds,
  );
  const inactiveSideBounds =
    presentation.inactiveOffsetXPx === null
      ? null
      : intersectBounds(
          {
            x: presentation.inactiveOffsetXPx,
            y: 0,
            width: presentation.activeWidthPx,
            height: fullHeight,
          },
          visibleOuterBounds,
        );
  return {
    activeBounds,
    inactiveSideBounds,
    visibleOuterBounds,
  };
}

function intersectBounds(
  first: CanvasBounds,
  second: CanvasBounds,
): CanvasBounds {
  const x = Math.max(first.x, second.x);
  const y = Math.max(first.y, second.y);
  const right = Math.min(
    first.x + first.width,
    second.x + second.width,
  );
  const bottom = Math.min(
    first.y + first.height,
    second.y + second.height,
  );
  return {
    x,
    y,
    width: Math.max(0, right - x),
    height: Math.max(0, bottom - y),
  };
}

export function createNormalCanvasLayout(
  sheets: readonly ComposedSheet[],
  bleedUm: number | undefined,
) {
  return createContinuousCanvasLayout(
    sheets,
    (sheet, presentation) =>
      createCanvasSheetViewGeometry(
        sheet,
        presentation,
        bleedUm,
        true,
      ).visibleOuterBounds.width,
  );
}

export function activePageHorizontalEdges(
  activeSides: ComposedSheet["activeSides"],
) {
  return {
    left: activeSides !== "right",
    right: activeSides !== "left",
  };
}
