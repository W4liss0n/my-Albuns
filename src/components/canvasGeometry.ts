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

export interface ContinuousCanvasEntry {
  sheetId: string;
  index: number;
  left: number;
  width: number;
  center: number;
  right: number;
}

export interface ContinuousCanvasLayout {
  readonly entries: readonly ContinuousCanvasEntry[];
  centeredOffset(
    sheetId: string,
    scale: number,
    canvasWidth: number,
  ): number | null;
  clampOffset(
    offsetX: number,
    scale: number,
    canvasWidth: number,
  ): number;
  centeredSheetId(
    offsetX: number,
    scale: number,
    canvasWidth: number,
  ): string | null;
}

export function createContinuousCanvasLayout(
  sheets: readonly ComposedSheet[],
): ContinuousCanvasLayout {
  let nextLeft = 0;
  const entries = sheets.map((sheet, index) => {
    const width =
      sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL;
    const entry: ContinuousCanvasEntry = {
      sheetId: sheet.sheetId,
      index,
      left: nextLeft,
      width,
      center: nextLeft + width / 2,
      right: nextLeft + width,
    };
    nextLeft += width + SHEET_GAP_PX;
    return entry;
  });
  const entriesBySheetId = new Map(
    entries.map((entry) => [entry.sheetId, entry]),
  );

  return {
    entries,
    centeredOffset(sheetId, scale, canvasWidth) {
      const entry = entriesBySheetId.get(sheetId);
      return entry
        ? canvasWidth / 2 - entry.center * scale
        : null;
    },
    clampOffset(offsetX, scale, canvasWidth) {
      const first = entries[0];
      const last = entries[entries.length - 1];
      if (!first || !last || scale <= 0) return offsetX;

      const maximum = canvasWidth / 2 - first.center * scale;
      const minimum = canvasWidth / 2 - last.center * scale;
      return Math.min(maximum, Math.max(minimum, offsetX));
    },
    centeredSheetId(offsetX, scale, canvasWidth) {
      if (entries.length === 0 || scale <= 0) return null;

      const visibleCenter = (canvasWidth / 2 - offsetX) / scale;
      let closest = entries[0];
      let closestDistance = Number.POSITIVE_INFINITY;
      for (const entry of entries) {
        const distance = Math.abs(entry.center - visibleCenter);
        if (distance < closestDistance) {
          closest = entry;
          closestDistance = distance;
        }
      }
      return closest.sheetId;
    },
  };
}
