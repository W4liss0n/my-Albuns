import { Container, Graphics, type FederatedPointerEvent } from "pixi.js";
import type { FrameResizeHandle } from "../domain/project";

import { pixiColor } from "./pixiColor";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";

const RESIZE_HANDLE_POSITIONS = [
  { name: "top-left", id: "topLeft", cursor: "nwse-resize", xRatio: 0, yRatio: 0 },
  { name: "top", id: "top", cursor: "ns-resize", xRatio: 0.5, yRatio: 0 },
  { name: "top-right", id: "topRight", cursor: "nesw-resize", xRatio: 1, yRatio: 0 },
  { name: "right", id: "right", cursor: "ew-resize", xRatio: 1, yRatio: 0.5 },
  { name: "bottom-right", id: "bottomRight", cursor: "nwse-resize", xRatio: 1, yRatio: 1 },
  { name: "bottom", id: "bottom", cursor: "ns-resize", xRatio: 0.5, yRatio: 1 },
  { name: "bottom-left", id: "bottomLeft", cursor: "nesw-resize", xRatio: 0, yRatio: 1 },
  { name: "left", id: "left", cursor: "ew-resize", xRatio: 0, yRatio: 0.5 },
] as const;

export interface FrameSelectionRenderNode {
  container: Container;
  resizeHandles: readonly Graphics[];
}

export function createFrameSelectionRenderNode(
  frameId: string,
  width: number,
  height: number,
  showResizeHandles: boolean,
  onResizeStart: (handle: FrameResizeHandle, event: FederatedPointerEvent) => void,
): FrameSelectionRenderNode {
  const handleStyle = SHEET_VISUAL_STYLE.frameSelection;
  const container = new Container();
  container.label = `frame-selection-container-${frameId}`;
  container.eventMode = "passive";
  container.visible = false;

  const outline = new Graphics()
    .rect(0, 0, width, height)
    .stroke(SHEET_VISUAL_STYLE.technicalOutlineStroke);
  outline.label = `frame-selection-${frameId}`;
  outline.eventMode = "none";
  container.addChild(outline);

  if (!showResizeHandles) {
    return { container, resizeHandles: [] };
  }

  const resizeHandles = RESIZE_HANDLE_POSITIONS.map(
    ({ name, id, cursor, xRatio, yRatio }) => {
      const halfSize = handleStyle.handleSizePx / 2;
      const handle = new Graphics()
        .rect(
          -halfSize,
          -halfSize,
          handleStyle.handleSizePx,
          handleStyle.handleSizePx,
        )
        .fill({ color: pixiColor(handleStyle.handleFill) })
        .stroke({
          alignment: 0,
          color: pixiColor(handleStyle.handleOutline),
          width: handleStyle.handleOutlineWidthPx,
          pixelLine: true,
        });
      handle.label =
        `frame-resize-handle-${name}-${frameId}`;
      handle.eventMode = "static";
      handle.cursor = cursor;
      handle.on("pointerdown", (event: FederatedPointerEvent) => onResizeStart(id, event));
      handle.on("pointertap", (event: FederatedPointerEvent) => event.stopPropagation());
      handle.position.set(width * xRatio, height * yRatio);
      container.addChild(handle);
      return handle;
    },
  );

  return { container, resizeHandles };
}

export function applyFrameSelectionScale(
  node: FrameSelectionRenderNode,
  canvasScale: number,
) {
  const inverseScale = 1 / Math.max(canvasScale, Number.EPSILON);
  for (const handle of node.resizeHandles) {
    handle.scale.set(inverseScale);
  }
}
