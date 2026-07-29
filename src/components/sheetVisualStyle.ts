export const SHEET_VISUAL_STYLE = {
  surface: {
    fill: "#f1ece2",
    outline: "#ffffff",
    outlineOpacity: 0.65,
    outlineWidthPx: 1,
    cornerRadiusPx: 3,
  },
  centerLine: {
    color: "#887b6c",
    opacity: 0.32,
    widthPx: 1,
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
    outline: "#d4b279",
    outlineOpacity: 0.45,
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
