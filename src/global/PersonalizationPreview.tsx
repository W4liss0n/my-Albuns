import type {
  BackgroundDraftContent,
  NewProjectPersonalizationDraft,
  OverlayDraftContent,
  PersonalizationScope,
} from "./application/newProjectPersonalization";
import { draftFrameBorderFillRects } from "./application/draftFrameBorderGeometry";
import { SheetGuideLayer } from "./SheetGuideLayer";

interface PersonalizationPreviewProps {
  bleedUm: number;
  focusedScope: NewProjectPersonalizationDraft["fixedScope"] | null;
  frameGapUm: number;
  heightUm: number;
  hoveredScope: NewProjectPersonalizationDraft["fixedScope"] | null;
  personalization: NewProjectPersonalizationDraft;
  safetyUm: number;
  widthUm: number;
}

export function PersonalizationPreview({
  bleedUm,
  focusedScope,
  frameGapUm,
  heightUm,
  hoveredScope,
  personalization,
  safetyUm,
  widthUm,
}: PersonalizationPreviewProps) {
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
  const focusStrokeWidth = Math.max(1, heightUm * 0.0035);
  const focusOutline = focusedScope
    ? scopeOutline(
        focusedScope,
        heightUm,
        pageWidth,
        widthUm,
        heightUm * 0.012,
      )
    : null;
  const hoverArea = hoveredScope
    ? scopeOutline(hoveredScope, heightUm, pageWidth, widthUm, 0)
    : null;

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
      {pageDescriptors.map(({ pageIndex, side, x: pageX }) => {
        const isSelected = scopeContainsPage(
          personalization.fixedScope,
          pageIndex,
        );
        const isFocused = focusedScope
          ? scopeContainsPage(focusedScope, pageIndex)
          : false;

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
                    aria-label={`Frame demonstrativo ${side} ${frameNumber}`}
                    fill="#7A684E"
                    fillOpacity={
                      isSelected ? "0.24" : isFocused ? "0.15" : "0.08"
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
                      aria-label={`Borda do Frame ${side} ${frameNumber}`}
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
      {pageDescriptors.map(({ pageIndex, side, x: pageX }) => {
        const isSelected = scopeContainsPage(
          personalization.fixedScope,
          pageIndex,
        );
        const isCandidate =
          (hoveredScope
            ? scopeContainsPage(hoveredScope, pageIndex)
            : false) ||
          (focusedScope
            ? scopeContainsPage(focusedScope, pageIndex)
            : false);

        return isSelected ? null : (
          <rect
            aria-label={`Atenuação do lado ${side}`}
            fill="#E3E0DA"
            fillOpacity={isCandidate ? "0.18" : "0.42"}
            height={heightUm}
            key={side}
            pointerEvents="none"
            stroke="none"
            width={pageWidth}
            x={pageX}
            y="0"
          />
        );
      })}
      {hoveredScope && hoverArea ? (
        <rect
          aria-label={scopeOutlineLabel("Pré-seleção", hoveredScope)}
          fill="var(--ui-text-muted)"
          fillOpacity="0.08"
          pointerEvents="none"
          stroke="none"
          {...hoverArea}
        />
      ) : null}
      {focusedScope && focusOutline ? (
        <rect
          aria-label={scopeOutlineLabel("Foco de teclado", focusedScope)}
          fill="none"
          pointerEvents="none"
          stroke="#73A9CE"
          strokeDasharray={`${focusStrokeWidth * 0.1} ${heightUm * 0.018}`}
          strokeLinecap="round"
          strokeWidth={focusStrokeWidth}
          {...focusOutline}
        />
      ) : null}
      <SheetGuideLayer
        bleedUm={bleedUm}
        heightUm={heightUm}
        safetyUm={safetyUm}
        widthUm={widthUm}
      />
    </svg>
  );
}

function scopeContainsPage(
  scope: PersonalizationScope,
  pageIndex: number,
) {
  const descriptor = SCOPE_DESCRIPTORS[scope];
  return (
    pageIndex >= descriptor.firstPageIndex &&
    pageIndex < descriptor.firstPageIndex + descriptor.pageCount
  );
}

function scopeOutline(
  scope: PersonalizationScope,
  heightUm: number,
  pageWidth: number,
  widthUm: number,
  inset: number,
) {
  const descriptor = SCOPE_DESCRIPTORS[scope];
  const scopeWidth = Math.min(
    widthUm,
    pageWidth * descriptor.pageCount,
  );
  const scopeX = pageWidth * descriptor.firstPageIndex;
  return {
    height: Math.max(1, heightUm - inset * 2),
    width: Math.max(1, scopeWidth - inset * 2),
    x: scopeX + inset,
    y: inset,
  };
}

function scopeOutlineLabel(
  prefix: "Foco de teclado" | "Pré-seleção",
  scope: PersonalizationScope,
) {
  return `${prefix} ${SCOPE_DESCRIPTORS[scope].labelSuffix}`;
}

const SCOPE_DESCRIPTORS = {
  both: {
    firstPageIndex: 0,
    labelSuffix: "de ambos os lados",
    pageCount: 2,
  },
  left: {
    firstPageIndex: 0,
    labelSuffix: "do lado esquerdo",
    pageCount: 1,
  },
  right: {
    firstPageIndex: 1,
    labelSuffix: "do lado direito",
    pageCount: 1,
  },
} as const satisfies Record<
  PersonalizationScope,
  { firstPageIndex: number; labelSuffix: string; pageCount: number }
>;

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
