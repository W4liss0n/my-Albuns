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
  frameGapPx: number;
  heightUm: number;
  personalization: NewProjectPersonalizationDraft;
  safetyUm: number;
  transientScope: NewProjectPersonalizationDraft["fixedScope"] | null;
  widthUm: number;
}

export function PersonalizationPreview({
  bleedUm,
  frameGapPx,
  heightUm,
  personalization,
  safetyUm,
  transientScope,
  widthUm,
}: PersonalizationPreviewProps) {
  const pageWidth = widthUm / 2;
  const frameInsetX = pageWidth * 0.04;
  const frameInsetY = heightUm * 0.04;
  const frameGap = pageWidth * (frameGapPx / 300);
  const frameWidth = (pageWidth - frameInsetX * 2 - frameGap) / 2;
  const frameHeight = heightUm - frameInsetY * 2;
  const frameBorder = personalization.frameBorder;
  const selectionStrokeWidth = Math.max(1, heightUm * 0.006);
  const selectionOutline = scopeOutline(
    personalization.fixedScope,
    heightUm,
    pageWidth,
    widthUm,
    selectionStrokeWidth / 2,
  );
  const transientStrokeWidth = Math.max(1, heightUm * 0.0035);
  const transientOutline = transientScope
    ? scopeOutline(
        transientScope,
        heightUm,
        pageWidth,
        widthUm,
        selectionStrokeWidth * 2,
      )
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
      {[0, pageWidth].map((pageX, pageIndex) => {
        const side = pageIndex === 0 ? "esquerdo" : "direito";
        const isSelected = scopeContainsPage(
          personalization.fixedScope,
          pageIndex,
        );
        const isTransient = transientScope
          ? scopeContainsPage(transientScope, pageIndex)
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
                      isSelected ? "0.24" : isTransient ? "0.15" : "0.08"
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
      {transientScope && transientOutline ? (
        <rect
          aria-label={scopeOutlineLabel("Realce temporário", transientScope)}
          fill="none"
          pointerEvents="none"
          stroke="#73A9CE"
          strokeDasharray={`${transientStrokeWidth * 0.1} ${heightUm * 0.018}`}
          strokeLinecap="round"
          strokeWidth={transientStrokeWidth}
          {...transientOutline}
        />
      ) : null}
      <rect
        aria-label={scopeOutlineLabel(
          "Seleção fixa",
          personalization.fixedScope,
        )}
        fill="none"
        pointerEvents="none"
        stroke="#2F7FBA"
        strokeWidth={selectionStrokeWidth}
        {...selectionOutline}
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
  prefix: "Realce temporário" | "Seleção fixa",
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
