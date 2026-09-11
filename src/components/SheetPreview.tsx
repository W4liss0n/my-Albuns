import { useId, type CSSProperties, type ReactNode } from "react";

import type {
  ComposedBackground,
  ComposedFrame,
  ComposedPhoto,
  ComposedSheet,
} from "../domain/project";
import { CANVAS_MICROMETERS_PER_PIXEL } from "./canvasGeometry";
import {
  frameOutlineStyle,
  inactiveSideCssGradient,
  SHEET_VISUAL_STYLE,
} from "./sheetVisualStyle";
import "./SheetPreview.css";
import { PHOTO_BLACK_AND_WHITE_SVG_MATRIX } from "./photoBlackAndWhite";

export interface SheetPreviewViewport {
  readonly xUm: number;
  readonly yUm: number;
  readonly widthUm: number;
  readonly heightUm: number;
}

interface SheetPreviewProps {
  sheet: ComposedSheet;
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  viewport?: SheetPreviewViewport;
}

interface SheetPreviewShellProps extends SheetPreviewProps {
  children?: ReactNode;
  className?: string;
}

type SheetPreviewShellStyle = CSSProperties & {
  "--sheet-inactive-side-gradient"?: string;
};

function sheetPreviewShellStyle(
  activeSides: ComposedSheet["activeSides"],
): SheetPreviewShellStyle {
  return activeSides === "both"
    ? {}
    : {
        "--sheet-inactive-side-gradient":
          inactiveSideCssGradient(activeSides),
      };
}

function sheetPreviewViewBox(
  sheet: ComposedSheet,
  viewport: SheetPreviewViewport | undefined,
) {
  return viewport
    ? `${viewport.xUm} ${viewport.yUm} ${viewport.widthUm} ${viewport.heightUm}`
    : `0 0 ${sheet.widthUm} ${sheet.heightUm}`;
}

export function SheetPreviewShell({
  children,
  className,
  mediaPreviewUrls,
  sheet,
  viewport,
}: SheetPreviewShellProps) {
  return (
    <SheetPreviewSurface activeSides={sheet.activeSides} className={className}>
      <SheetPreview mediaPreviewUrls={mediaPreviewUrls} sheet={sheet} viewport={viewport} />
      {children}
    </SheetPreviewSurface>
  );
}

export function SheetPreviewSurface({ activeSides, className, children }: {
  activeSides: ComposedSheet["activeSides"];
  className?: string;
  children: ReactNode;
}) {
  return (
    <span
      className={
        className
          ? `sheet-preview-shell ${className}`
          : "sheet-preview-shell"
      }
      data-active-sides={activeSides}
      style={sheetPreviewShellStyle(activeSides)}
    >
      {children}
    </span>
  );
}

export function SheetPreview({
  sheet,
  mediaPreviewUrls = {},
  viewport,
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
      viewBox={sheetPreviewViewBox(sheet, viewport)}
      xmlns="http://www.w3.org/2000/svg"
    >
      <title>{label}</title>
      <defs>
        {sheet.backgrounds.map((background, index) => background.kind === "media" && background.clipRect ? (
          <clipPath id={`${instanceId}-background-${index}`} key={`background-${index}`} clipPathUnits="userSpaceOnUse">
            <rect {...background.clipRect} />
          </clipPath>
        ) : null)}
        {sheet.overlays.map((overlay, index) => overlay.clipRect ? (
          <clipPath id={`${instanceId}-overlay-${index}`} key={`overlay-${index}`} clipPathUnits="userSpaceOnUse">
            <rect {...overlay.clipRect} />
          </clipPath>
        ) : null)}
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
          clipPath={background.kind === "media" && background.clipRect ? `url(#${instanceId}-background-${index})` : undefined}
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
          key={frame.frameId}
          previewUrl={
            frame.photo
              ? mediaPreviewUrls[frame.photo.mediaId]
              : undefined
          }
          unit={unit}
        />
      ))}

      {sheet.overlays.map((overlay, index) =>
        mediaPreviewUrls[overlay.mediaId] ? (
          <image
            data-preview-overlay-id={overlay.mediaId}
            href={mediaPreviewUrls[overlay.mediaId]}
            key={`${overlay.mediaId}-${index}`}
            clipPath={overlay.clipRect ? `url(#${instanceId}-overlay-${index})` : undefined}
            x={overlay.drawRect.x}
            y={overlay.drawRect.y}
            width={overlay.drawRect.width}
            height={overlay.drawRect.height}
            preserveAspectRatio="none"
          />
        ) : (
          <rect
            data-preview-overlay-id={overlay.mediaId}
            key={`${overlay.mediaId}-${index}`}
            clipPath={overlay.clipRect ? `url(#${instanceId}-overlay-${index})` : undefined}
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
  clipPath,
  previewUrl,
}: {
  background: ComposedBackground;
  clipPath?: string;
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
      clipPath={clipPath}
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
      clipPath={clipPath}
      fill={SHEET_VISUAL_STYLE.mediaFallback.fill}
      x={drawRect.x}
      y={drawRect.y}
      width={drawRect.width}
      height={drawRect.height}
    />
  );
}

interface FramePreviewProps {
  frame: ComposedFrame;
  clipId: string;
  previewUrl?: string;
  unit: number;
}

function FramePreview({
  frame,
  clipId: frameClipId,
  previewUrl,
  unit,
}: FramePreviewProps) {
  const { clipRect, photo, border: frameBorder } = frame;
  const placeholderStyle = SHEET_VISUAL_STYLE.framePlaceholder;
  const outlineStyle = frameOutlineStyle(photo !== null);

  return (
    <g>
      <g data-preview-frame-content-id={frame.frameId} opacity={frame.opacityByte / 255}>
      {photo ? (
        <g clipPath={`url(#${frameClipId})`}>
          <PhotoPreview
            photo={photo}
            previewUrl={previewUrl}
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
          />
        </g>
      )}
      {frameBorder.kind === "solid" && frame.borderFillRects.length > 0 ? (
        <g
          data-preview-frame-border-id={frame.frameId}
          pointerEvents="none"
        >
          {frame.borderFillRects.map((rect, index) => (
            <rect
              data-preview-frame-border-segment={index}
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
      <rect
        data-preview-frame-id={frame.frameId}
        x={clipRect.x}
        y={clipRect.y}
        width={clipRect.width}
        height={clipRect.height}
        fill="none"
        stroke={outlineStyle.outline}
        strokeOpacity={outlineStyle.outlineOpacity}
        strokeWidth={outlineStyle.outlineWidthPx * unit}
      />
    </g>
  );
}

interface PhotoPreviewProps {
  photo: ComposedPhoto;
  previewUrl?: string;
}

function PhotoPreview({
  photo,
  previewUrl,
}: PhotoPreviewProps) {
  const effectId = `photo-black-white-${useId().replace(/[^a-zA-Z0-9_-]/g, "")}`;
  const { drawRect } = photo;
  const centerX = drawRect.x + drawRect.width / 2;
  const centerY = drawRect.y + drawRect.height / 2;
  const transform = [
    `translate(${centerX} ${centerY})`,
    `scale(${photo.mirrorX ? -1 : 1} 1)`,
    `rotate(${photo.rotationDegrees})`,
    `translate(${-centerX} ${-centerY})`,
  ].join(" ");

  return (
    <g
      data-preview-photo-id={photo.mediaId}
      transform={transform}
      filter={photo.blackAndWhite ? `url(#${effectId})` : undefined}
    >
      {photo.blackAndWhite && (
        <defs>
          <filter id={effectId} colorInterpolationFilters="sRGB">
            <feColorMatrix type="matrix" values={PHOTO_BLACK_AND_WHITE_SVG_MATRIX} />
          </filter>
        </defs>
      )}
      {previewUrl ? (
        <image
          href={previewUrl}
          height={drawRect.height}
          preserveAspectRatio="none"
          width={drawRect.width}
          x={drawRect.x}
          y={drawRect.y}
        />
      ) : null}
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
