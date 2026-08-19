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
    fill: "#f4f1eb",
    foldShadow: "#8a847a",
    foldShadowOpacity: 0.09,
    foldShadowWidthPx: 12,
    foldShadowSteps: 6,
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
  placeholder: {
    fill: "#ded8cc",
    outline: "#b9b1a4",
    outlineOpacity: 0.8,
    outlineWidthPx: 1,
    crossColor: "#948b7e",
    crossOpacity: 0.75,
    crossWidthPx: 1.4,
    crossHalfLengthPx: 12,
  },
  canvasPlaceholder: {
    light: "#e6e1d7",
    dark: "#dcd6ca",
    stripeWidthPx: 4,
    stripeGapPx: 4,
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

export function photoPaletteIndexForStripe(stripe: number) {
  return Math.min(
    2,
    Math.floor(
      (stripe / SHEET_VISUAL_STYLE.photo.stripeCount) * 3,
    ),
  );
}
