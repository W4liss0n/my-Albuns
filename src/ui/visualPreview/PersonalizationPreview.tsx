import { renderableMediaPreviewUrl } from "../../application/mediaPreviews";
import type {
  PreviewBackgroundContent,
  PreviewOverlayContent,
  VisualPersonalizationPreview,
  VisualPreviewGeometry,
} from "./types";
import { draftFrameBorderFillRects } from "./draftFrameBorderGeometry";
import { SheetGuideLayer } from "./SheetGuideLayer";
import {
  scaleVisualMediaFallbackLength,
  VISUAL_MEDIA_FALLBACK_STYLE,
} from "./visualMediaFallbackStyle";
import "./VisualPreviewSheet.css";

interface PersonalizationPreviewProps {
  accessibleLabel: string;
  frameOpacities: readonly number[];
  frameGapUm: number;
  geometry: VisualPreviewGeometry;
  personalization: Omit<VisualPersonalizationPreview, "fixedScope">;
  showTechnicalGuides: boolean;
}

export function PersonalizationPreview({
  accessibleLabel,
  frameOpacities,
  frameGapUm,
  geometry,
  personalization,
  showTechnicalGuides,
}: PersonalizationPreviewProps) {
  const { heightUm, widthUm } = geometry;
  const pageWidth = widthUm / 2;
  const pageDescriptors = [
    { pageIndex: 0, side: "esquerdo", x: 0 },
    { pageIndex: 1, side: "direito", x: pageWidth },
  ] as const;
  const frameInsetX = pageWidth * 0.04;
  const frameInsetY = heightUm * 0.04;
  const frameGap = Math.min(
    frameGapUm,
    Math.max(0, pageWidth - frameInsetX * 2),
  );
  const frameWidth = Math.max(
    0,
    (pageWidth - frameInsetX * 2 - frameGap) / 2,
  );
  const frameHeight = heightUm - frameInsetY * 2;
  const frameBorder = personalization.frameBorder;
  return (
    <svg
      aria-label={accessibleLabel}
      className="visual-preview-sheet"
      height={heightUm}
      preserveAspectRatio="xMidYMid meet"
      role="img"
      viewBox={`0 0 ${widthUm} ${heightUm}`}
      width={widthUm}
      xmlns="http://www.w3.org/2000/svg"
    >
      <title>{accessibleLabel}</title>
      <rect
        aria-label="Fundo branco"
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
          label="Fundo de ambos os lados"
          width={widthUm}
          x={0}
        />
      ) : (
        <>
          <BackgroundContent
            content={personalization.background.left}
            height={heightUm}
            label="Fundo do lado esquerdo"
            width={pageWidth}
            x={0}
          />
          <BackgroundContent
            content={personalization.background.right}
            height={heightUm}
            label="Fundo do lado direito"
            width={pageWidth}
            x={pageWidth}
          />
        </>
      )}
      {pageDescriptors.map(({ pageIndex, side, x: pageX }) => {
        return (
          <g key={side}>
            {[0, 1].map((frameIndex) => {
              const frameNumber = frameIndex + 1;
              const x =
                pageX +
                frameInsetX +
                frameIndex * (frameWidth + frameGap);
              const borderFillRects =
                frameBorder.kind === "solid"
                  ? draftFrameBorderFillRects(
                      {
                        height: frameHeight,
                        width: frameWidth,
                        x,
                        y: frameInsetY,
                      },
                      frameBorder.widthUm,
                    )
                  : [];

              return (
                <g key={frameNumber}>
                  <rect
                    aria-label={`Quadro de exemplo ${frameNumber}, lado ${side}`}
                    fill="#7A684E"
                    fillOpacity={
                      frameOpacities[pageIndex]
                    }
                    height={frameHeight}
                    pointerEvents="none"
                    stroke="none"
                    width={frameWidth}
                    x={x}
                    y={frameInsetY}
                  />
                  {frameBorder.kind === "solid" && borderFillRects.length > 0 ? (
                    <g
                      aria-label={`Borda do quadro ${side} ${frameNumber}`}
                      pointerEvents="none"
                    >
                      {borderFillRects.map((rect, index) => (
                        <rect
                          data-border-segment={index}
                          fill={frameBorder.rgb}
                          height={rect.height}
                          key={index}
                          width={rect.width}
                          x={rect.x}
                          y={rect.y}
                        />
                      ))}
                    </g>
                  ) : null}
                </g>
              );
            })}
          </g>
        );
      })}
      {personalization.overlay.scope === "bothSides" ? (
        <OverlayContent
          content={personalization.overlay.both}
          height={heightUm}
          label="Sobreposição de ambos os lados"
          width={widthUm}
          x={0}
        />
      ) : (
        <>
          <OverlayContent
            content={personalization.overlay.left}
            height={heightUm}
            label="Sobreposição do lado esquerdo"
            width={pageWidth}
            x={0}
          />
          <OverlayContent
            content={personalization.overlay.right}
            height={heightUm}
            label="Sobreposição do lado direito"
            width={pageWidth}
            x={pageWidth}
          />
        </>
      )}
      {showTechnicalGuides ? <SheetGuideLayer geometry={geometry} /> : null}
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
  content: PreviewOverlayContent;
  height: number;
  label: string;
  width: number;
  x: number;
}) {
  if (!content) return null;
  const previewUrl = renderableMediaPreviewUrl(content.preview);
  return previewUrl ? (
    <image
      aria-label={label}
      data-preview-state={content.preview.state}
      height={height}
      href={previewUrl}
      preserveAspectRatio="none"
      width={width}
      x={x}
      y="0"
    />
  ) : (
    <rect
      aria-label={label}
      data-preview-state={content.preview.state}
      fill="none"
      height={height}
      rx={scaleVisualMediaFallbackLength(
        height,
        VISUAL_MEDIA_FALLBACK_STYLE.overlay.cornerRadiusPx,
      )}
      stroke={VISUAL_MEDIA_FALLBACK_STYLE.overlay.outline}
      strokeOpacity={VISUAL_MEDIA_FALLBACK_STYLE.overlay.outlineOpacity}
      strokeWidth={Math.max(
        1,
        scaleVisualMediaFallbackLength(
          height,
          VISUAL_MEDIA_FALLBACK_STYLE.overlay.outlineWidthPx,
        ),
      )}
      width={width}
      x={x}
      y="0"
    />
  );
}

function BackgroundContent({
  content,
  height,
  label,
  width,
  x,
}: {
  content: PreviewBackgroundContent;
  height: number;
  label: string;
  width: number;
  x: number;
}) {
  if (content.kind === "color") {
    return (
      <rect
        aria-label={label}
        fill={content.rgb}
        height={height}
        width={width}
        x={x}
        y="0"
      />
    );
  }
  const previewUrl = renderableMediaPreviewUrl(content.preview);
  return previewUrl ? (
    <image
      aria-label={label}
      data-preview-state={content.preview.state}
      height={height}
      href={previewUrl}
      preserveAspectRatio="none"
      width={width}
      x={x}
      y="0"
    />
  ) : (
    <rect
      aria-label={label}
      data-preview-state={content.preview.state}
      fill={VISUAL_MEDIA_FALLBACK_STYLE.background.fill}
      height={height}
      width={width}
      x={x}
      y="0"
    />
  );
}
