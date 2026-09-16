import { CANVAS_VERTICAL_MARGIN_PX } from "./canvasGeometry";

export interface ViewPoint { x: number; y: number }
export interface EditingCanvasFit {
  width: number;
  height: number;
  sheetWidth: number;
  sheetHeight: number;
  scale: number;
}
export interface EditingCanvasTransform extends ViewPoint { zoom: number }

/** Coordinates belong to the Canvas in CSS pixels, independently of render resolution. */
export function fittedEditingTransform(fit: EditingCanvasFit): EditingCanvasTransform {
  return { zoom: 1, x: (fit.width - fit.sheetWidth * fit.scale) / 2,
    y: (fit.height - fit.sheetHeight * fit.scale) / 2 };
}

export function boundedEditingTransform(fit: EditingCanvasFit, transform: EditingCanvasTransform): EditingCanvasTransform {
  const zoom = Math.max(1, Math.min(4, transform.zoom));
  if (zoom === 1) return fittedEditingTransform(fit);
  const bound = (offset: number, available: number, content: number) => {
    const margin = CANVAS_VERTICAL_MARGIN_PX;
    // Keep an edge reachable without forcing a smaller axis back to its center.
    return Math.max(margin - content, Math.min(available - margin, offset));
  };
  return { zoom, x: bound(transform.x, fit.width, fit.sheetWidth * fit.scale * zoom),
    y: bound(transform.y, fit.height, fit.sheetHeight * fit.scale * zoom) };
}

export function zoomEditingTransform(fit: EditingCanvasFit, transform: EditingCanvasTransform,
  factor: number, anchor: ViewPoint): EditingCanvasTransform {
  const zoom = Math.max(1, Math.min(4, transform.zoom * factor));
  if (zoom === 1) return fittedEditingTransform(fit);
  const ratio = zoom / transform.zoom;
  // Position clamping here would move the point under the cursor. Only Pan and
  // viewport resize bound translation; returning to minimum restores full fit.
  return { zoom,
    x: anchor.x - (anchor.x - transform.x) * ratio,
    y: anchor.y - (anchor.y - transform.y) * ratio };
}

export function resizedEditingTransform(previous: EditingCanvasFit, fit: EditingCanvasFit,
  transform: EditingCanvasTransform): EditingCanvasTransform {
  const ratio = fit.scale / previous.scale;
  return boundedEditingTransform(fit, { zoom: transform.zoom,
    x: fit.width / 2 - (previous.width / 2 - transform.x) * ratio,
    y: fit.height / 2 - (previous.height / 2 - transform.y) * ratio });
}
