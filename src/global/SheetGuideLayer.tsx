import type { NewProjectPreviewGeometry } from "./newProjectPreviewGeometry";

interface SheetGuideLayerProps {
  geometry: NewProjectPreviewGeometry;
}

export function SheetGuideLayer({
  geometry: { bleedUm, heightUm, safetyUm, widthUm },
}: SheetGuideLayerProps) {
  const smallestSide = Math.min(widthUm, heightUm);
  const bleed = Math.max(0, Math.min(bleedUm, smallestSide / 4));
  const safety = Math.max(0, Math.min(safetyUm, smallestSide / 4));
  const safetyInset = bleed + safety;
  const dashLength = Math.max(2, heightUm * 0.01);
  const dashGap = Math.max(2, heightUm * 0.008);
  const strokeWidth = Math.max(1, heightUm * 0.003);

  return (
    <g
      aria-label="Guias técnicas da Lâmina"
      pointerEvents="none"
      role="group"
    >
      <line
        stroke="#d7d2c8"
        strokeWidth={strokeWidth}
        x1={widthUm / 2}
        x2={widthUm / 2}
        y1={0}
        y2={heightUm}
      />
      <rect
        fill="none"
        height={Math.max(1, heightUm - bleed * 2)}
        stroke="#c57c70"
        strokeDasharray={`${dashLength} ${dashGap}`}
        strokeWidth={strokeWidth}
        width={Math.max(1, widthUm - bleed * 2)}
        x={bleed}
        y={bleed}
      />
      <rect
        fill="none"
        height={Math.max(1, heightUm - safetyInset * 2)}
        stroke="#6f9fbe"
        strokeDasharray={`${dashLength} ${dashGap}`}
        strokeWidth={strokeWidth}
        width={Math.max(1, widthUm - safetyInset * 2)}
        x={safetyInset}
        y={safetyInset}
      />
    </g>
  );
}
