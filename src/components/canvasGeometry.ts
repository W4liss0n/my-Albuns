import type { ComposedSheet } from "../domain/project";

export const MICROMETER_TO_CANVAS_PIXEL = 0.001;
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
