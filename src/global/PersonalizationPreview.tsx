import type {
  BackgroundDraftContent,
  NewProjectPersonalizationDraft,
  OverlayDraftContent,
} from "./application/newProjectPersonalization";

interface PersonalizationPreviewProps {
  heightUm: number;
  personalization: NewProjectPersonalizationDraft;
  widthUm: number;
}

export function PersonalizationPreview({
  heightUm,
  personalization,
  widthUm,
}: PersonalizationPreviewProps) {
  const pageWidth = widthUm / 2;
  const frameInsetX = widthUm * 0.06;
  const frameInsetY = heightUm * 0.12;
  const frameWidth = pageWidth - frameInsetX * 2;
  const frameHeight = heightUm - frameInsetY * 2;
  const frameBorder = personalization.frameBorder;
  const highlightedScope =
    personalization.hoveredScope ?? personalization.fixedScope;

  return (
    <svg
      aria-label="Reprodução da Lâmina"
      className="new-project-preview-sheet"
      preserveAspectRatio="xMidYMid meet"
      role="img"
      style={{ aspectRatio: `${widthUm} / ${heightUm}` }}
      viewBox={`0 0 ${widthUm} ${heightUm}`}
      xmlns="http://www.w3.org/2000/svg"
    >
      <title>Reprodução da Lâmina</title>
      <rect
        aria-label="Base branca canônica"
        fill="#FFFFFF"
        height={heightUm}
        pointerEvents="none"
        width={widthUm}
        x="0"
        y="0"
      />
      {personalization.background.scope === "bothSides" ? (
        <BackgroundContent
          content={personalization.background.both}
          height={heightUm}
          label="Background de ambos os lados"
          width={widthUm}
          x={0}
        />
      ) : (
        <>
          <BackgroundContent
            content={personalization.background.left}
            height={heightUm}
            label="Background do lado esquerdo"
            width={pageWidth}
            x={0}
          />
          <BackgroundContent
            content={personalization.background.right}
            height={heightUm}
            label="Background do lado direito"
            width={pageWidth}
            x={pageWidth}
          />
        </>
      )}
      <line
        stroke="#D6D0C4"
        strokeWidth={Math.max(1, heightUm * 0.002)}
        x1={pageWidth}
        x2={pageWidth}
        y1="0"
        y2={heightUm}
      />
      {personalization.overlay.scope === "bothSides" ? (
        <OverlayContent
          content={personalization.overlay.both}
          height={heightUm}
          label="Overlay de ambos os lados"
          width={widthUm}
          x={0}
        />
      ) : (
        <>
          <OverlayContent
            content={personalization.overlay.left}
            height={heightUm}
            label="Overlay do lado esquerdo"
            width={pageWidth}
            x={0}
          />
          <OverlayContent
            content={personalization.overlay.right}
            height={heightUm}
            label="Overlay do lado direito"
            width={pageWidth}
            x={pageWidth}
          />
        </>
      )}
      {[0, pageWidth].map((pageX, index) => {
        const side = index === 0 ? "esquerdo" : "direito";
        const x = pageX + frameInsetX;

        return (
          <g key={pageX}>
            <rect
              aria-label={`Frame demonstrativo ${side}`}
              fill="none"
              height={frameHeight}
              stroke="#81796D"
              strokeDasharray={`${heightUm * 0.012} ${heightUm * 0.008}`}
              strokeOpacity="0.72"
              strokeWidth={Math.max(1, heightUm * 0.002)}
              width={frameWidth}
              x={x}
              y={frameInsetY}
            />
            {frameBorder.kind === "solid" ? (
              <rect
                aria-label={`Borda do Frame ${side}`}
                fill="none"
                height={frameHeight}
                stroke={frameBorder.rgb}
                strokeWidth={frameBorder.widthUm}
                width={frameWidth}
                x={x}
                y={frameInsetY}
              />
            ) : null}
          </g>
        );
      })}
      <rect
        aria-label={
          highlightedScope === "both"
            ? "Realce de ambos os lados"
            : highlightedScope === "left"
              ? "Realce do lado esquerdo"
              : "Realce do lado direito"
        }
        fill="#D3AB77"
        fillOpacity="0.18"
        height={heightUm}
        pointerEvents="none"
        width={highlightedScope === "both" ? widthUm : pageWidth}
        x={highlightedScope === "right" ? pageWidth : 0}
        y="0"
      />
    </svg>
  );
}

function OverlayContent({
  content,
  height,
  label,
  width,
  x,
}: {
  content: OverlayDraftContent;
  height: number;
  label: string;
  width: number;
  x: number;
}) {
  return content ? (
    <image
      aria-label={label}
      height={height}
      href={content.selection.previewUrl}
      preserveAspectRatio="none"
      width={width}
      x={x}
      y="0"
    />
  ) : null;
}

function BackgroundContent({
  content,
  height,
  label,
  width,
  x,
}: {
  content: BackgroundDraftContent;
  height: number;
  label: string;
  width: number;
  x: number;
}) {
  return content.kind === "color" ? (
    <rect
      aria-label={label}
      fill={content.rgb}
      height={height}
      width={width}
      x={x}
      y="0"
    />
  ) : (
    <image
      aria-label={label}
      height={height}
      href={content.selection.previewUrl}
      preserveAspectRatio="none"
      width={width}
      x={x}
      y="0"
    />
  );
}
