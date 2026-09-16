/** Shared visual policy for image drags, independent of DOM or Canvas rendering. */
export const IMAGE_DRAG_GHOST_STYLE = {
  maxWidthPx: 80,
  maxHeightPx: 60,
  opacity: 0.84,
  pointerOffsetPx: 6,
  viewportInsetPx: 8,
  border: { widthPx: 2, color: "#ffffff" },
  shadow: { xPx: 3, yPx: 5, expansionPx: 2, color: "#252525", opacity: 0.2 },
} as const;

export function imageDragGhostGeometry(sourceWidth: number | null, sourceHeight: number | null) {
  const style = IMAGE_DRAG_GHOST_STYLE;
  const knownSize = sourceWidth !== null && sourceHeight !== null && sourceWidth > 0 && sourceHeight > 0;
  const source = knownSize ? { width: sourceWidth, height: sourceHeight }
    : { width: style.maxHeightPx, height: style.maxHeightPx };
  const scale = Math.min(1, style.maxWidthPx / source.width, style.maxHeightPx / source.height);
  const width = source.width * scale, height = source.height * scale;
  const border = style.border.widthPx, shadow = style.shadow;
  return {
    width, height, scale,
    border: { x: -border, y: -border, width: width + border * 2, height: height + border * 2 },
    shadow: { x: shadow.xPx, y: shadow.yPx, width: width + shadow.expansionPx, height: height + shadow.expansionPx },
  };
}

export function imageDragGhostPosition(
  pointer: { x: number; y: number },
  size: { width: number; height: number },
  viewport: { width: number; height: number },
) {
  const { pointerOffsetPx: offset, viewportInsetPx: inset } = IMAGE_DRAG_GHOST_STYLE;
  return {
    x: Math.max(inset, Math.min(pointer.x + offset, viewport.width - size.width - inset)),
    y: Math.max(inset, Math.min(pointer.y + offset, viewport.height - size.height - inset)),
  };
}
