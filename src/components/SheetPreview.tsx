import { useId } from "react";

import type {
  ComposedBackground,
  ComposedFrame,
  ComposedPhoto,
  ComposedSheet,
  ProjectedFrameBorder,
} from "../domain/project";
import { CANVAS_MICROMETERS_PER_PIXEL } from "./canvasGeometry";
import {
  photoPaletteIndexForStripe,
  SHEET_VISUAL_STYLE,
} from "./sheetVisualStyle";

interface SheetPreviewProps {
  sheet: ComposedSheet;
  frameBorder?: ProjectedFrameBorder;
  mediaPreviewUrls?: Readonly<Record<string, string>>;
}

export function SheetPreview({
  sheet,
  frameBorder = { kind: "none" },
  mediaPreviewUrls = {},
}: SheetPreviewProps) {
  const instanceId = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const label = `Prévia da Lâmina ${String(sheet.number).padStart(2, "0")}`;
  const unit = CANVAS_MICROMETERS_PER_PIXEL;
  const surfaceStyle = SHEET_VISUAL_STYLE.surface;
  const frames = sheet.frames;

  return (
    <svg
      aria-label={label}
      className="sheet-preview"
      focusable="false"
      preserveAspectRatio="xMidYMid meet"
      role="img"
      viewBox={`0 0 ${sheet.widthUm} ${sheet.heightUm}`}
      xmlns="http://www.w3.org/2000/svg"
    >
      <title>{label}</title>
      <defs>
        {frames.map((frame, index) => (
          <clipPath
            id={clipId(instanceId, frame, index)}
            key={frame.frameId}
            clipPathUnits="userSpaceOnUse"
          >
            <rect
              x={frame.clipRect.x}
              y={frame.clipRect.y}
              width={frame.clipRect.width}
              height={frame.clipRect.height}
            />
          </clipPath>
        ))}
      </defs>

      <rect
        x="0"
        y="0"
        width={sheet.widthUm}
        height={sheet.heightUm}
        rx={surfaceStyle.cornerRadiusPx * unit}
        fill={sheet.base.rgb}
        stroke={surfaceStyle.outline}
        strokeOpacity={surfaceStyle.outlineOpacity}
        strokeWidth={surfaceStyle.outlineWidthPx * unit}
      />
      {sheet.backgrounds.map((background, index) => (
        <BackgroundPreview
          background={background}
          key={`${background.kind}-${index}`}
          previewUrl={
            background.kind === "media"
              ? mediaPreviewUrls[background.mediaId]
              : undefined
          }
        />
      ))}
      {sheet.activeSides === "both" ? (
        <line
          x1={sheet.widthUm / 2}
          y1="0"
          x2={sheet.widthUm / 2}
          y2={sheet.heightUm}
          stroke={SHEET_VISUAL_STYLE.centerLine.color}
          strokeOpacity={SHEET_VISUAL_STYLE.centerLine.opacity}
          strokeWidth={SHEET_VISUAL_STYLE.centerLine.widthPx * unit}
        />
      ) : null}

      {frames.map((frame, index) => (
        <FramePreview
          clipId={clipId(instanceId, frame, index)}
          frame={frame}
          frameBorder={frameBorder}
          key={frame.frameId}
          previewUrl={
            frame.photo
              ? mediaPreviewUrls[frame.photo.mediaId]
              : undefined
          }
          unit={unit}
        />
      ))}

      {sheet.overlays.map((overlay) =>
        mediaPreviewUrls[overlay.mediaId] ? (
          <image
            data-preview-overlay-id={overlay.mediaId}
            href={mediaPreviewUrls[overlay.mediaId]}
            key={overlay.mediaId}
            x={overlay.drawRect.x}
            y={overlay.drawRect.y}
            width={overlay.drawRect.width}
            height={overlay.drawRect.height}
            preserveAspectRatio="none"
          />
        ) : (
          <rect
            data-preview-overlay-id={overlay.mediaId}
            key={overlay.mediaId}
            x={overlay.drawRect.x}
            y={overlay.drawRect.y}
            width={overlay.drawRect.width}
            height={overlay.drawRect.height}
            rx={SHEET_VISUAL_STYLE.overlay.cornerRadiusPx * unit}
            fill="none"
            stroke={SHEET_VISUAL_STYLE.overlay.outline}
            strokeOpacity={SHEET_VISUAL_STYLE.overlay.outlineOpacity}
            strokeWidth={
              SHEET_VISUAL_STYLE.overlay.outlineWidthPx * unit
            }
          />
        ),
      )}
    </svg>
  );
}

function BackgroundPreview({
  background,
  previewUrl,
}: {
  background: ComposedBackground;
  previewUrl?: string;
}) {
  const { drawRect } = background;
  if (background.kind === "color") {
    return (
      <rect
        data-preview-background-color={background.rgb}
        fill={background.rgb}
        x={drawRect.x}
        y={drawRect.y}
        width={drawRect.width}
        height={drawRect.height}
      />
    );
  }
  return previewUrl ? (
    <image
      data-preview-background-id={background.mediaId}
      href={previewUrl}
      preserveAspectRatio="none"
      x={drawRect.x}
      y={drawRect.y}
      width={drawRect.width}
      height={drawRect.height}
    />
  ) : (
    <rect
      data-preview-background-id={background.mediaId}
      fill="#D8DEE2"
      x={drawRect.x}
      y={drawRect.y}
      width={drawRect.width}
      height={drawRect.height}
    />
  );
}

interface FramePreviewProps {
  frame: ComposedFrame;
  frameBorder: ProjectedFrameBorder;
  clipId: string;
  previewUrl?: string;
  unit: number;
}

function FramePreview({
  frame,
  frameBorder,
  clipId: frameClipId,
  previewUrl,
  unit,
}: FramePreviewProps) {
  const { clipRect, photo } = frame;
  const placeholderStyle = SHEET_VISUAL_STYLE.placeholder;

  return (
    <g>
      {photo ? (
        <g clipPath={`url(#${frameClipId})`}>
          <PhotoPreview
            photo={photo}
            previewUrl={previewUrl}
            unit={unit}
          />
        </g>
      ) : (
        <g data-preview-placeholder-id={frame.frameId}>
          <rect
            x={clipRect.x}
            y={clipRect.y}
            width={clipRect.width}
            height={clipRect.height}
            fill={placeholderStyle.fill}
            stroke={placeholderStyle.outline}
            strokeOpacity={placeholderStyle.outlineOpacity}
            strokeWidth={placeholderStyle.outlineWidthPx * unit}
          />
          <line
            x1={
              clipRect.x +
              clipRect.width / 2 -
              placeholderStyle.crossHalfLengthPx * unit
            }
            y1={clipRect.y + clipRect.height / 2}
            x2={
              clipRect.x +
              clipRect.width / 2 +
              placeholderStyle.crossHalfLengthPx * unit
            }
            y2={clipRect.y + clipRect.height / 2}
            stroke={placeholderStyle.crossColor}
            strokeOpacity={placeholderStyle.crossOpacity}
            strokeWidth={placeholderStyle.crossWidthPx * unit}
          />
          <line
            x1={clipRect.x + clipRect.width / 2}
            y1={
              clipRect.y +
              clipRect.height / 2 -
              placeholderStyle.crossHalfLengthPx * unit
            }
            x2={clipRect.x + clipRect.width / 2}
            y2={
              clipRect.y +
              clipRect.height / 2 +
              placeholderStyle.crossHalfLengthPx * unit
            }
            stroke={placeholderStyle.crossColor}
            strokeOpacity={placeholderStyle.crossOpacity}
            strokeWidth={placeholderStyle.crossWidthPx * unit}
          />
        </g>
      )}
      <rect
        data-preview-frame-id={frame.frameId}
        x={clipRect.x}
        y={clipRect.y}
        width={clipRect.width}
        height={clipRect.height}
        fill="none"
        stroke={SHEET_VISUAL_STYLE.frame.outline}
        strokeOpacity={SHEET_VISUAL_STYLE.frame.outlineOpacity}
        strokeWidth={SHEET_VISUAL_STYLE.frame.outlineWidthPx * unit}
      />
      {frameBorder.kind === "solid" ? (
        <rect
          data-preview-frame-border-id={frame.frameId}
          fill="none"
          height={clipRect.height}
          pointerEvents="none"
          stroke={frameBorder.rgb}
          strokeWidth={frameBorder.widthUm}
          width={clipRect.width}
          x={clipRect.x}
          y={clipRect.y}
        />
      ) : null}
    </g>
  );
}

interface PhotoPreviewProps {
  photo: ComposedPhoto;
  previewUrl?: string;
  unit: number;
}

function PhotoPreview({
  photo,
  previewUrl,
  unit,
}: PhotoPreviewProps) {
  const { drawRect } = photo;
  const photoStyle = SHEET_VISUAL_STYLE.photo;
  const centerX = drawRect.x + drawRect.width / 2;
  const centerY = drawRect.y + drawRect.height / 2;
  const transform = [
    `translate(${centerX} ${centerY})`,
    `rotate(${photo.rotationDegrees})`,
    `scale(${photo.mirrorX ? -1 : 1} 1)`,
    `translate(${-centerX} ${-centerY})`,
  ].join(" ");

  return (
    <g
      data-preview-photo-id={photo.mediaId}
      transform={transform}
    >
      {previewUrl ? (
        <image
          href={previewUrl}
          height={drawRect.height}
          preserveAspectRatio="none"
          width={drawRect.width}
          x={drawRect.x}
          y={drawRect.y}
        />
      ) : (
        <>
          {Array.from(
            { length: photoStyle.stripeCount },
            (_, stripe) => {
              const paletteIndex =
                photoPaletteIndexForStripe(stripe);
              const stripeWidth =
                drawRect.width / photoStyle.stripeCount;

              return (
                <rect
                  fill={photo.palette[paletteIndex]}
                  height={drawRect.height}
                  key={stripe}
                  width={
                    stripeWidth +
                    photoStyle.stripeOverlapPx * unit
                  }
                  x={drawRect.x + stripeWidth * stripe}
                  y={drawRect.y}
                />
              );
            },
          )}
          <circle
            cx={
              drawRect.x +
              drawRect.width * photoStyle.lightCenterXRatio
            }
            cy={
              drawRect.y +
              drawRect.height * photoStyle.lightCenterYRatio
            }
            r={
              drawRect.height *
              photoStyle.lightRadiusToHeightRatio
            }
            fill={photoStyle.lightColor}
            fillOpacity={photoStyle.lightOpacity}
          />
        </>
      )}
    </g>
  );
}

function clipId(
  instanceId: string,
  frame: ComposedFrame,
  index: number,
) {
  const frameId = frame.frameId.replace(/[^a-zA-Z0-9_-]/g, "");
  return `sheet-preview-${instanceId}-${frameId}-${index}`;
}
