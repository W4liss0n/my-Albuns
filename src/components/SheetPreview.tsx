import { useId } from "react";

import type {
  ComposedFrame,
  ComposedPhoto,
  ComposedSheet,
} from "../domain/project";
import {
  photoPaletteIndexForStripe,
  SHEET_VISUAL_STYLE,
} from "./sheetVisualStyle";

interface SheetPreviewProps {
  sheet: ComposedSheet;
}

export function SheetPreview({ sheet }: SheetPreviewProps) {
  const instanceId = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const label = `Prévia da Lâmina ${String(sheet.number).padStart(2, "0")}`;
  const unit = sheet.heightUm / 300;
  const surfaceStyle = SHEET_VISUAL_STYLE.surface;
  const frames = [...sheet.frames].sort(
    (first, second) => first.zIndex - second.zIndex,
  );

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
        fill={surfaceStyle.fill}
        stroke={surfaceStyle.outline}
        strokeOpacity={surfaceStyle.outlineOpacity}
        strokeWidth={surfaceStyle.outlineWidthPx * unit}
      />
      <line
        x1={sheet.widthUm / 2}
        y1="0"
        x2={sheet.widthUm / 2}
        y2={sheet.heightUm}
        stroke={SHEET_VISUAL_STYLE.centerLine.color}
        strokeOpacity={SHEET_VISUAL_STYLE.centerLine.opacity}
        strokeWidth={SHEET_VISUAL_STYLE.centerLine.widthPx * unit}
      />

      {frames.map((frame, index) => (
        <FramePreview
          clipId={clipId(instanceId, frame, index)}
          frame={frame}
          key={frame.frameId}
          unit={unit}
        />
      ))}

      {sheet.hasOverlay && (
        <rect
          data-preview-overlay=""
          x={SHEET_VISUAL_STYLE.overlay.insetPx * unit}
          y={SHEET_VISUAL_STYLE.overlay.insetPx * unit}
          width={Math.max(
            0,
            sheet.widthUm -
              SHEET_VISUAL_STYLE.overlay.insetPx * 2 * unit,
          )}
          height={Math.max(
            0,
            sheet.heightUm -
              SHEET_VISUAL_STYLE.overlay.insetPx * 2 * unit,
          )}
          rx={SHEET_VISUAL_STYLE.overlay.cornerRadiusPx * unit}
          fill="none"
          stroke={SHEET_VISUAL_STYLE.overlay.outline}
          strokeOpacity={SHEET_VISUAL_STYLE.overlay.outlineOpacity}
          strokeWidth={
            SHEET_VISUAL_STYLE.overlay.outlineWidthPx * unit
          }
        />
      )}
    </svg>
  );
}

interface FramePreviewProps {
  frame: ComposedFrame;
  clipId: string;
  unit: number;
}

function FramePreview({
  frame,
  clipId: frameClipId,
  unit,
}: FramePreviewProps) {
  const { clipRect, photo } = frame;
  const placeholderStyle = SHEET_VISUAL_STYLE.placeholder;

  return (
    <g>
      {photo ? (
        <g clipPath={`url(#${frameClipId})`}>
          <PhotoPreview photo={photo} unit={unit} />
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
    </g>
  );
}

interface PhotoPreviewProps {
  photo: ComposedPhoto;
  unit: number;
}

function PhotoPreview({ photo, unit }: PhotoPreviewProps) {
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
      {Array.from({ length: photoStyle.stripeCount }, (_, stripe) => {
        const paletteIndex = photoPaletteIndexForStripe(stripe);
        const stripeWidth = drawRect.width / photoStyle.stripeCount;

        return (
          <rect
            fill={photo.palette[paletteIndex]}
            height={drawRect.height}
            key={stripe}
            width={stripeWidth + photoStyle.stripeOverlapPx * unit}
            x={drawRect.x + stripeWidth * stripe}
            y={drawRect.y}
          />
        );
      })}
      <circle
        cx={
          drawRect.x +
          drawRect.width * photoStyle.lightCenterXRatio
        }
        cy={
          drawRect.y +
          drawRect.height * photoStyle.lightCenterYRatio
        }
        r={drawRect.height * photoStyle.lightRadiusToHeightRatio}
        fill={photoStyle.lightColor}
        fillOpacity={photoStyle.lightOpacity}
      />
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
