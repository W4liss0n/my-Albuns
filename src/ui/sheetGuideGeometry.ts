export const SHEET_GUIDE_STYLE = {
  bleed: "#c57c70",
  safety: "#6f9fbe",
  opacity: 0.9,
} as const;

interface SheetGuideGeometryInput {
  bleedUm: number;
  heightUm: number;
  safetyUm: number;
}

export function createSheetGuideGeometry({
  bleedUm,
  heightUm,
  safetyUm,
}: SheetGuideGeometryInput) {
  const bleedInsetUm = Math.max(0, bleedUm);
  const safetyDistanceUm = Math.max(0, safetyUm);
  return {
    bleedInsetUm,
    dashGapUm: Math.max(2, heightUm * 0.008),
    dashLengthUm: Math.max(2, heightUm * 0.01),
    safetyInsetUm: bleedInsetUm + safetyDistanceUm,
    strokeWidthUm: Math.max(1, heightUm * 0.003),
  };
}
