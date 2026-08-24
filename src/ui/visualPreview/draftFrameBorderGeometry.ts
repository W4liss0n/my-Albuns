/** Geometry used by the shared visual-personalization renderer. */
export interface DraftFrameRect {
  height: number;
  width: number;
  x: number;
  y: number;
}

export function draftFrameBorderFillRects(
  frame: DraftFrameRect,
  requestedWidth: number,
): DraftFrameRect[] {
  const width = Math.min(
    Number.isFinite(requestedWidth) ? Math.max(0, requestedWidth) : 0,
    Math.max(0, frame.width),
    Math.max(0, frame.height),
  );
  if (width <= 0) {
    return [];
  }

  return [
    { x: frame.x, y: frame.y, width: frame.width, height: width },
    {
      x: frame.x,
      y: frame.y + frame.height - width,
      width: frame.width,
      height: width,
    },
    { x: frame.x, y: frame.y, width, height: frame.height },
    {
      x: frame.x + frame.width - width,
      y: frame.y,
      width,
      height: frame.height,
    },
  ];
}
