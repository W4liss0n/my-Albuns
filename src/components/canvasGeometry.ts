import type { ComposedSheet } from "../domain/project";

export const CANVAS_MICROMETERS_PER_PIXEL = 1_000;
export const MICROMETER_TO_CANVAS_PIXEL =
  1 / CANVAS_MICROMETERS_PER_PIXEL;
export const SHEET_GAP_PX = 52;
export const SHEET_LABEL_HEIGHT_PX = 24;
export const CANVAS_VERTICAL_MARGIN_PX = 24;

export function continuousCanvasScale(
  canvasHeight: number,
  sheetHeight: number,
): number {
  const availableHeight = Math.max(
    1,
    canvasHeight - CANVAS_VERTICAL_MARGIN_PX * 2,
  );
  return availableHeight / (sheetHeight + SHEET_LABEL_HEIGHT_PX);
}

export function sheetOffsetInCanvasPixels(
  sheets: readonly ComposedSheet[],
  index: number,
): number {
  return sheets
    .slice(0, index)
    .reduce(
      (offset, sheet) =>
        offset +
        sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL +
        SHEET_GAP_PX,
      0,
    );
}

export function centeredSheetOffsetInContinuousCanvas(
  sheets: readonly ComposedSheet[],
  index: number,
  scale: number,
  canvasWidth: number,
): number {
  return (
    canvasWidth / 2 -
    sheetCenterInCanvasPixels(sheets, index) * scale
  );
}

export function clampContinuousCanvasOffset(
  sheets: readonly ComposedSheet[],
  offsetX: number,
  scale: number,
  canvasWidth: number,
): number {
  if (sheets.length === 0 || scale <= 0) return offsetX;

  const lastIndex = sheets.length - 1;
  const maximum = centeredSheetOffsetInContinuousCanvas(
    sheets,
    0,
    scale,
    canvasWidth,
  );
  const minimum = centeredSheetOffsetInContinuousCanvas(
    sheets,
    lastIndex,
    scale,
    canvasWidth,
  );

  return Math.min(maximum, Math.max(minimum, offsetX));
}

export function centeredSheetIdInContinuousCanvas(
  sheets: readonly ComposedSheet[],
  offsetX: number,
  scale: number,
  canvasWidth: number,
): string | null {
  if (sheets.length === 0 || scale <= 0) return null;

  const visibleCenter = (canvasWidth / 2 - offsetX) / scale;
  let closestIndex = 0;
  let closestDistance = Number.POSITIVE_INFINITY;

  sheets.forEach((_sheet, index) => {
    const distance = Math.abs(
      sheetCenterInCanvasPixels(sheets, index) - visibleCenter,
    );
    if (distance < closestDistance) {
      closestIndex = index;
      closestDistance = distance;
    }
  });

  return sheets[closestIndex].sheetId;
}

function sheetCenterInCanvasPixels(
  sheets: readonly ComposedSheet[],
  index: number,
): number {
  return (
    sheetOffsetInCanvasPixels(sheets, index) +
    sheets[index].widthUm * MICROMETER_TO_CANVAS_PIXEL / 2
  );
}
