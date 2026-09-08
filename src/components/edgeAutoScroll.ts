export const EDGE_AUTO_SCROLL = {
  edgeMarginPx: 72,
  maxVelocityPxPerSecond: {
    horizontal: 960,
    vertical: 720,
  },
} as const;

export interface EdgeAutoScrollInput {
  readonly axis: "horizontal" | "vertical";
  readonly pointerPosition: number;
  readonly viewportStart: number;
  readonly viewportEnd: number;
}

export function edgeAutoScrollVelocity({
  axis,
  pointerPosition,
  viewportStart,
  viewportEnd,
}: EdgeAutoScrollInput): number {
  if (
    !Number.isFinite(pointerPosition) ||
    !Number.isFinite(viewportStart) ||
    !Number.isFinite(viewportEnd) ||
    viewportEnd <= viewportStart
  ) {
    return 0;
  }

  const viewportLength = viewportEnd - viewportStart;
  const edgeMargin = Math.min(
    EDGE_AUTO_SCROLL.edgeMarginPx,
    viewportLength / 2,
  );
  if (edgeMargin <= 0) {
    return 0;
  }

  const maximumVelocity =
    EDGE_AUTO_SCROLL.maxVelocityPxPerSecond[axis];
  const leadingThreshold = viewportStart + edgeMargin;
  if (pointerPosition < leadingThreshold) {
    const proximity = clampUnit(
      (leadingThreshold - pointerPosition) / edgeMargin,
    );
    return -maximumVelocity * proximity * proximity;
  }

  const trailingThreshold = viewportEnd - edgeMargin;
  if (pointerPosition > trailingThreshold) {
    const proximity = clampUnit(
      (pointerPosition - trailingThreshold) / edgeMargin,
    );
    return maximumVelocity * proximity * proximity;
  }

  return 0;
}

function clampUnit(value: number): number {
  return Math.min(1, Math.max(0, value));
}
