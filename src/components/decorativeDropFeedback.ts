import { Container, Graphics } from "pixi.js";
import type { DecorativeDropPreview } from "../domain/project";
import type { CanvasBounds } from "./canvasSheetViewGeometry";
import { MICROMETER_TO_CANVAS_PIXEL } from "./canvasGeometry";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";

/** The pointer zone chooses a scope; the outline shows the actual affected Pages. */
export function createDecorativeDropFeedback(preview: DecorativeDropPreview, activeBounds: CanvasBounds) {
  const container = new Container();
  container.eventMode = "none";
  const halo = new Graphics();
  const target = new Graphics();
  const center = new Graphics();
  target.label = `decorative-drop-target-${preview.sheet.sheetId}`;
  center.label = `decorative-drop-center-${preview.sheet.sheetId}`;
  container.addChild(halo, target, center);

  const bounds = { ...activeBounds };
  if (preview.sheet.activeSides === "both" && preview.scope !== "bothSides") {
    const middle = preview.sheet.widthUm * MICROMETER_TO_CANVAS_PIXEL / 2;
    if (preview.scope === "right") {
      bounds.width = bounds.x + bounds.width - middle;
      bounds.x = middle;
    } else bounds.width = middle - bounds.x;
  }

  return {
    container,
    applyScale(scale: number) {
      const color = SHEET_VISUAL_STYLE.technicalOutlineStroke.color;
      for (const [outline, strokeColor, width] of [[halo, 0xffffff, 4], [target, color, 2]] as const) {
        outline.clear().rect(bounds.x, bounds.y, bounds.width, bounds.height)
          .stroke({ color: strokeColor, width: width / scale, alignment: 1 });
      }
      center.clear();
      if (preview.centerRect) {
        const left = preview.centerRect.x * MICROMETER_TO_CANVAS_PIXEL;
        const right = (preview.centerRect.x + preview.centerRect.width) * MICROMETER_TO_CANVAS_PIXEL;
        const top = bounds.y + 8 / scale;
        const bottom = bounds.y + bounds.height - 8 / scale;
        const tick = 6 / scale;
        center.moveTo(left, top + tick).lineTo(left, top).lineTo(right, top).lineTo(right, top + tick)
          .moveTo(left, bottom - tick).lineTo(left, bottom).lineTo(right, bottom).lineTo(right, bottom - tick)
          .stroke({ color, alpha: preview.scope === "bothSides" ? 0.8 : 0.45, pixelLine: true });
      }
    },
  };
}
