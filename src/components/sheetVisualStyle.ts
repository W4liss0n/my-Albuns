export const SHEET_VISUAL_STYLE = {
  surface: {
    fill: "#f3f1ec",
    outline: "#d8d3c9",
    outlineOpacity: 0.85,
    outlineWidthPx: 1,
    cornerRadiusPx: 3,
  },
  centerLine: {
    color: "#887b6c",
    opacity: 0.32,
    widthPx: 1,
  },
  sheetBar: {
    heightPx: 40,
    surface: "#fbfaf8",
    surfaceOpacity: 0.96,
    separator: "#e9e5dc",
    separatorOpacity: 1,
    text: "#8a847a",
    action: "#6d675d",
    hoverOpacity: 0.55,
    hoverTransitionDurationMs: 140,
    hoverTransitionFrameMs: 16,
    placeholderActionOpacity: 0.45,
    pageFontSizePx: 12.5,
    numberFontSizePx: 12.5,
  },
  inactiveSide: {
    fill: "#d8d4cc",
    opacity: 0.88,
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
