import type { ComposedSheet } from "../domain/project";

export const CANVAS_MICROMETERS_PER_PIXEL = 1_000;
export const MICROMETER_TO_CANVAS_PIXEL =
  1 / CANVAS_MICROMETERS_PER_PIXEL;
export const SHEET_GAP_PX = 46;
export const CANVAS_VERTICAL_MARGIN_PX = 28;

export interface CanvasSheetPresentation {
  activeOffsetXPx: number;
  activeWidthPx: number;
  inactiveOffsetXPx: number | null;
  visualWidthPx: number;
}

export function createCanvasSheetPresentation(
  sheet: Pick<ComposedSheet, "activeSides" | "widthUm">,
): CanvasSheetPresentation {
  const activeWidthPx = sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL;
  if (sheet.activeSides === "both") {
    return {
      activeOffsetXPx: 0,
      activeWidthPx,
      inactiveOffsetXPx: null,
      visualWidthPx: activeWidthPx,
    };
  }

  return {
    activeOffsetXPx: sheet.activeSides === "right" ? activeWidthPx : 0,
    activeWidthPx,
    inactiveOffsetXPx:
      sheet.activeSides === "right" ? 0 : activeWidthPx,
    visualWidthPx: activeWidthPx * 2,
  };
}

export function continuousCanvasScale(
  canvasHeight: number,
  sheetHeight: number,
): number {
  const availableHeight = Math.max(
    1,
    canvasHeight - CANVAS_VERTICAL_MARGIN_PX * 2,
  );
  return availableHeight / sheetHeight;
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
  entriesAtScale(scale: number): readonly ContinuousCanvasEntry[];
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
  visualWidthForSheet: (
    sheet: ComposedSheet,
    presentation: CanvasSheetPresentation,
  ) => number = (_sheet, presentation) => presentation.visualWidthPx,
): ContinuousCanvasLayout {
  const measuredSheets = sheets.map((sheet, index) => {
    const presentation = createCanvasSheetPresentation(sheet);
    return {
      sheetId: sheet.sheetId,
      index,
      width: visualWidthForSheet(sheet, presentation),
    };
  });
  const entriesAtScale = (scale: number) => {
    const safeScale = Math.max(scale, Number.EPSILON);
    const gap = SHEET_GAP_PX / safeScale;
    let nextLeft = 0;
    return measuredSheets.map((sheet) => {
      const entry: ContinuousCanvasEntry = {
        ...sheet,
        left: nextLeft,
        center: nextLeft + sheet.width / 2,
        right: nextLeft + sheet.width,
      };
      nextLeft += sheet.width + gap;
      return entry;
    });
  };

  return {
    entriesAtScale,
    centeredOffset(sheetId, scale, canvasWidth) {
      const entries = entriesAtScale(scale);
      const entriesBySheetId = new Map(
        entries.map((entry) => [entry.sheetId, entry]),
      );
      const entry = entriesBySheetId.get(sheetId);
      return entry
        ? canvasWidth / 2 - entry.center * scale
        : null;
    },
    clampOffset(offsetX, scale, canvasWidth) {
      const entries = entriesAtScale(scale);
      const first = entries[0];
      const last = entries[entries.length - 1];
      if (!first || !last || scale <= 0) return offsetX;

      const maximum = canvasWidth / 2 - first.center * scale;
      const minimum = canvasWidth / 2 - last.center * scale;
      return Math.min(maximum, Math.max(minimum, offsetX));
    },
    centeredSheetId(offsetX, scale, canvasWidth) {
      const entries = entriesAtScale(scale);
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
