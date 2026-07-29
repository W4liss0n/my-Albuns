import { useId } from "react";

import type {
  ComposedFrame,
  ComposedPhoto,
  ComposedSheet,
} from "../domain/project";

interface SheetPreviewProps {
  sheet: ComposedSheet;
}

const PHOTO_STRIPE_COUNT = 12;

export function SheetPreview({ sheet }: SheetPreviewProps) {
  const instanceId = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const label = `Prévia da Lâmina ${String(sheet.number).padStart(2, "0")}`;
  const unit = sheet.heightUm / 300;
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
        rx={3 * unit}
        fill="#f1ece2"
        stroke="#ffffff"
        strokeOpacity="0.65"
        strokeWidth={unit}
      />
      <line
        x1={sheet.widthUm / 2}
        y1="0"
        x2={sheet.widthUm / 2}
        y2={sheet.heightUm}
        stroke="#887b6c"
        strokeOpacity="0.32"
        strokeWidth={unit}
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
          x={8 * unit}
          y={8 * unit}
          width={Math.max(0, sheet.widthUm - 16 * unit)}
          height={Math.max(0, sheet.heightUm - 16 * unit)}
          rx={2 * unit}
          fill="none"
          stroke="#d4b279"
          strokeOpacity="0.45"
          strokeWidth={2 * unit}
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
            fill="#ded8cc"
            stroke="#b9b1a4"
            strokeOpacity="0.8"
            strokeWidth={unit}
          />
          <line
            x1={clipRect.x + clipRect.width / 2 - 12 * unit}
            y1={clipRect.y + clipRect.height / 2}
            x2={clipRect.x + clipRect.width / 2 + 12 * unit}
            y2={clipRect.y + clipRect.height / 2}
            stroke="#948b7e"
            strokeOpacity="0.75"
            strokeWidth={1.4 * unit}
          />
          <line
            x1={clipRect.x + clipRect.width / 2}
            y1={clipRect.y + clipRect.height / 2 - 12 * unit}
            x2={clipRect.x + clipRect.width / 2}
            y2={clipRect.y + clipRect.height / 2 + 12 * unit}
            stroke="#948b7e"
            strokeOpacity="0.75"
            strokeWidth={1.4 * unit}
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
        stroke="#ffffff"
        strokeOpacity="0.86"
        strokeWidth={unit}
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
      {Array.from({ length: PHOTO_STRIPE_COUNT }, (_, stripe) => {
        const paletteIndex = Math.min(
          2,
          Math.floor((stripe / PHOTO_STRIPE_COUNT) * 3),
        );
        const stripeWidth = drawRect.width / PHOTO_STRIPE_COUNT;

        return (
          <rect
            fill={photo.palette[paletteIndex]}
            height={drawRect.height}
            key={stripe}
            width={stripeWidth + unit}
            x={drawRect.x + stripeWidth * stripe}
            y={drawRect.y}
          />
        );
      })}
      <circle
        cx={drawRect.x + drawRect.width * 0.73}
        cy={drawRect.y + drawRect.height * 0.28}
        r={drawRect.height * 0.18}
        fill="#fff3d0"
        fillOpacity="0.32"
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
