export const SHEET_VISUAL_STYLE = {
  surface: {
    fill: "#f3f1ec",
    outline: "#d8d3c9",
    outlineOpacity: 0.85,
    outlineWidthPx: 1,
    cornerRadiusPx: 3,
  },
  centerLine: {
    color: "#eeeae1",
    opacity: 1,
    widthPx: 1,
  },
  bleedMask: {
    fill: "#f3f1ec",
    opacity: 1,
  },
  canvasShadow: {
    close: {
      color: "#3c362c",
      offsetYPx: 1,
      opacity: 0.16,
      spreadPx: 1.5,
      steps: 2,
    },
    depth: {
      color: "#3c362c",
      offsetYPx: 5,
      opacity: 0.12,
      spreadPx: 8,
      steps: 6,
    },
  },
  sheetBar: {
    heightPx: 40,
    surface: "#fbfaf8",
    surfaceOpacity: 0.96,
    separator: "#e9e5dc",
    separatorOpacity: 1,
    text: "#8a847a",
    action: "#6d675d",
    actionHover: "#2c2924",
    actionSizePx: 26,
    actionHoverOpacity: 1,
    sheetHoverOpacity: 0.55,
    directHoverOpacity: 1,
    hoverTransitionDurationMs: 140,
    hoverTransitionFrameMs: 16,
    placeholderActionOpacity: 0.45,
    pageFontSizePx: 12.5,
    numberFontSizePx: 12.5,
  },
  inactiveSide: {
    outerEdge: "#faf9f6",
    body: "#f4f1eb",
    bodyStopOffset: 0.58,
    fold: "#ece7df",
  },
  photo: {
    stripeCount: 12,
    stripeOverlapPx: 1,
    lightColor: "#fff3d0",
    lightOpacity: 0.32,
    lightCenterXRatio: 0.73,
    lightCenterYRatio: 0.28,
    lightRadiusToHeightRatio: 0.18,
  },
  framePlaceholder: {
    fill: "#ece8e1",
    outline: "#c9c2b7",
    outlineOpacity: 0.88,
    outlineWidthPx: 1,
    labelText: "#655f56",
    labelFontSizePx: 10.5,
  },
  frame: {
    outline: "#ffffff",
    outlineOpacity: 0.72,
    outlineWidthPx: 1,
  },
  overlay: {
    outline: "#2f7fba",
    outlineOpacity: 0.52,
    outlineWidthPx: 2,
    insetPx: 8,
    cornerRadiusPx: 2,
  },
} as const;

export function frameOutlineStyle(hasPhoto: boolean) {
  return hasPhoto
    ? SHEET_VISUAL_STYLE.frame
    : SHEET_VISUAL_STYLE.framePlaceholder;
}

const INACTIVE_SIDE_GRADIENT_ORIENTATION = {
  left: { cssDirection: "to left", startX: 1, endX: 0 },
  right: { cssDirection: "to right", startX: 0, endX: 1 },
} as const;

export function inactiveSideGradientOrientation(
  activeSides: "left" | "right",
) {
  return INACTIVE_SIDE_GRADIENT_ORIENTATION[activeSides];
}

export function inactiveSideCssGradient(
  activeSides: "left" | "right",
) {
  const style = SHEET_VISUAL_STYLE.inactiveSide;
  const { cssDirection } = inactiveSideGradientOrientation(activeSides);
  const bodyStopPercent = Number(
    (style.bodyStopOffset * 100).toFixed(4),
  );
  return `linear-gradient(${cssDirection}, ${style.outerEdge} 0%, ${style.body} ${bodyStopPercent}%, ${style.fold} 100%)`;
}

export function photoPaletteIndexForStripe(stripe: number) {
  return Math.min(
    2,
    Math.floor(
      (stripe / SHEET_VISUAL_STYLE.photo.stripeCount) * 3,
    ),
  );
}
