import type {
  BackgroundDraftContent,
  NewProjectPersonalizationDraft,
  OverlayDraftContent,
} from "./application/newProjectPersonalization";
import { SheetGuideLayer } from "./SheetGuideLayer";

interface PersonalizationPreviewProps {
  bleedUm: number;
  heightUm: number;
  personalization: NewProjectPersonalizationDraft;
  safetyUm: number;
  widthUm: number;
}

export function PersonalizationPreview({
  bleedUm,
  heightUm,
  personalization,
  safetyUm,
  widthUm,
}: PersonalizationPreviewProps) {
  const pageWidth = widthUm / 2;
  const frameInsetX = pageWidth * 0.04;
  const frameInsetY = heightUm * 0.04;
  const frameGap = pageWidth * 0.02;
  const frameWidth = (pageWidth - frameInsetX * 2 - frameGap) / 2;
  const frameHeight = heightUm - frameInsetY * 2;
  const frameBorder = personalization.frameBorder;
  const highlightedScope =
    personalization.hoveredScope ?? personalization.fixedScope;
  const highlightWidth =
    highlightedScope === "both" ? widthUm : pageWidth;
  const highlightX = highlightedScope === "right" ? pageWidth : 0;
  const highlightStrokeWidth = Math.max(1, heightUm * 0.006);
  const highlightInset = highlightStrokeWidth / 2;

  return (
    <svg
      aria-label="Reprodução da Lâmina"
      className="new-project-sheet new-project-personalization-sheet"
      height={heightUm}
      preserveAspectRatio="xMidYMid meet"
      role="img"
      viewBox={`0 0 ${widthUm} ${heightUm}`}
      width={widthUm}
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
      {[0, pageWidth].map((pageX, pageIndex) => {
        const side = pageIndex === 0 ? "esquerdo" : "direito";

        return (
          <g key={side}>
            {[0, 1].map((frameIndex) => {
              const frameNumber = frameIndex + 1;
              const x =
                pageX +
                frameInsetX +
                frameIndex * (frameWidth + frameGap);

              return (
                <g key={frameNumber}>
                  <rect
                    aria-label={`Frame demonstrativo ${side} ${frameNumber}`}
                    fill="#5B554C"
                    fillOpacity="0.08"
                    height={frameHeight}
                    pointerEvents="none"
                    stroke="none"
                    width={frameWidth}
                    x={x}
                    y={frameInsetY}
                  />
                  {frameBorder.kind === "solid" ? (
                    <rect
                      aria-label={`Borda do Frame ${side} ${frameNumber}`}
                      fill="none"
                      height={frameHeight}
                      pointerEvents="none"
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
        fill="none"
        height={Math.max(1, heightUm - highlightStrokeWidth)}
        pointerEvents="none"
        stroke="#2F7FBA"
        strokeWidth={highlightStrokeWidth}
        width={Math.max(1, highlightWidth - highlightStrokeWidth)}
        x={highlightX + highlightInset}
        y={highlightInset}
      />
      <SheetGuideLayer
        bleedUm={bleedUm}
        heightUm={heightUm}
        safetyUm={safetyUm}
        widthUm={widthUm}
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
