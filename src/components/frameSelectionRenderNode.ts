import { Container, Graphics } from "pixi.js";

import { pixiColor } from "./pixiColor";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";

const RESIZE_HANDLE_POSITIONS = [
  { name: "top-left", xRatio: 0, yRatio: 0 },
  { name: "top", xRatio: 0.5, yRatio: 0 },
  { name: "top-right", xRatio: 1, yRatio: 0 },
  { name: "right", xRatio: 1, yRatio: 0.5 },
  { name: "bottom-right", xRatio: 1, yRatio: 1 },
  { name: "bottom", xRatio: 0.5, yRatio: 1 },
  { name: "bottom-left", xRatio: 0, yRatio: 1 },
  { name: "left", xRatio: 0, yRatio: 0.5 },
] as const;

export interface FrameSelectionRenderNode {
  container: Container;
  resizeHandlePlaceholders: readonly Graphics[];
}

export function createFrameSelectionRenderNode(
  frameId: string,
  width: number,
  height: number,
  showResizeHandlePlaceholders: boolean,
): FrameSelectionRenderNode {
  const style = SHEET_VISUAL_STYLE.frameSelection;
  const container = new Container();
  container.label = `frame-selection-container-${frameId}`;
  container.eventMode = "none";
  container.visible = false;

  const outline = new Graphics()
    .rect(0, 0, width, height)
    .stroke({
      alignment: 0,
      color: pixiColor(style.outline),
      width: style.outlineWidthPx,
      alpha: style.outlineOpacity,
    });
  outline.label = `frame-selection-${frameId}`;
  outline.eventMode = "none";
  container.addChild(outline);

  if (!showResizeHandlePlaceholders) {
    return { container, resizeHandlePlaceholders: [] };
  }

  // Visual-only placeholder: these handles intentionally have no pointer
  // events until the Frame resize gesture and its domain mutation exist.
  const resizeHandlePlaceholders = RESIZE_HANDLE_POSITIONS.map(
    ({ name, xRatio, yRatio }) => {
      const halfSize = style.handleSizePx / 2;
      const handle = new Graphics()
        .rect(
          -halfSize,
          -halfSize,
          style.handleSizePx,
          style.handleSizePx,
        )
        .fill({ color: pixiColor(style.handleFill) })
        .stroke({
          alignment: 0,
          color: pixiColor(style.handleOutline),
          width: style.handleOutlineWidthPx,
          pixelLine: true,
        });
      handle.label =
        `frame-resize-handle-placeholder-${name}-${frameId}`;
      handle.eventMode = "none";
      handle.position.set(width * xRatio, height * yRatio);
      container.addChild(handle);
      return handle;
    },
  );

  return { container, resizeHandlePlaceholders };
}

export function applyFrameSelectionScale(
  node: FrameSelectionRenderNode,
  canvasScale: number,
) {
  const inverseScale = 1 / Math.max(canvasScale, Number.EPSILON);
  for (const handle of node.resizeHandlePlaceholders) {
    handle.scale.set(inverseScale);
  }
}
