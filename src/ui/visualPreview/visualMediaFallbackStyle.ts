const VISUAL_PREVIEW_REFERENCE_HEIGHT_PX = 300;

export const VISUAL_MEDIA_FALLBACK_STYLE = {
  background: {
    fill: "#D8DEE2",
  },
  overlay: {
    cornerRadiusPx: 2,
    outline: "#2f7fba",
    outlineOpacity: 0.52,
    outlineWidthPx: 2,
  },
} as const;

export function scaleVisualMediaFallbackLength(
  previewHeight: number,
  lengthPx: number,
) {
  return Math.round(
    (previewHeight * lengthPx) / VISUAL_PREVIEW_REFERENCE_HEIGHT_PX,
  );
}
