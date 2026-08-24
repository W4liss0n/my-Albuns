import {
  createSheetGuideGeometry,
  SHEET_GUIDE_STYLE,
} from "../ui/sheetGuideGeometry";
import type { VisualPreviewGeometry } from "../application/visualPersonalizationPreview";

interface SheetGuideLayerProps {
  geometry: VisualPreviewGeometry;
}

export function SheetGuideLayer({
  geometry: { bleedUm, heightUm, safetyUm, widthUm },
}: SheetGuideLayerProps) {
  const guideGeometry = createSheetGuideGeometry({
    bleedUm,
    heightUm,
    safetyUm,
  });

  return (
    <g
      aria-label="Guias técnicas da Lâmina"
      pointerEvents="none"
      role="group"
    >
      <line
        stroke="#d7d2c8"
        strokeWidth={guideGeometry.strokeWidthUm}
        x1={widthUm / 2}
        x2={widthUm / 2}
        y1={0}
        y2={heightUm}
      />
      {bleedUm > 0 ? (
        <rect
          fill="none"
          height={Math.max(1, heightUm - guideGeometry.bleedInsetUm * 2)}
          stroke={SHEET_GUIDE_STYLE.bleed}
          strokeDasharray={`${guideGeometry.dashLengthUm} ${guideGeometry.dashGapUm}`}
          strokeOpacity={SHEET_GUIDE_STYLE.opacity}
          strokeWidth={guideGeometry.strokeWidthUm}
          width={Math.max(1, widthUm - guideGeometry.bleedInsetUm * 2)}
          x={guideGeometry.bleedInsetUm}
          y={guideGeometry.bleedInsetUm}
        />
      ) : null}
      {safetyUm > 0 ? (
        <rect
          fill="none"
          height={Math.max(1, heightUm - guideGeometry.safetyInsetUm * 2)}
          stroke={SHEET_GUIDE_STYLE.safety}
          strokeDasharray={`${guideGeometry.dashLengthUm} ${guideGeometry.dashGapUm}`}
          strokeOpacity={SHEET_GUIDE_STYLE.opacity}
          strokeWidth={guideGeometry.strokeWidthUm}
          width={Math.max(1, widthUm - guideGeometry.safetyInsetUm * 2)}
          x={guideGeometry.safetyInsetUm}
          y={guideGeometry.safetyInsetUm}
        />
      ) : null}
    </g>
  );
}
