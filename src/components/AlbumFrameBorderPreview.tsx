import type { ProjectedFrameBorder } from "../domain/project";
import { draftFrameBorderFillRects } from "../global/application/draftFrameBorderGeometry";

const FRAME = { x: 14_000, y: 9_000, width: 192_000, height: 52_000 };

export function AlbumFrameBorderPreview({
  frameBorder,
}: {
  frameBorder: ProjectedFrameBorder;
}) {
  const segments =
    frameBorder.kind === "solid"
      ? draftFrameBorderFillRects(FRAME, frameBorder.widthUm)
      : [];

  return (
    <svg
      aria-label="Prévia da Borda dos Frames"
      className="album-frame-border-preview"
      role="img"
      viewBox="0 0 220000 70000"
    >
      <rect
        fill="var(--ui-surface-raised)"
        height={FRAME.height}
        width={FRAME.width}
        x={FRAME.x}
        y={FRAME.y}
      />
      <rect
        fill="var(--ui-canvas)"
        height={FRAME.height - 12_000}
        width={FRAME.width - 24_000}
        x={FRAME.x + 12_000}
        y={FRAME.y + 6_000}
      />
      {segments.map((segment, index) => (
        <rect
          data-border-segment={index}
          fill={frameBorder.kind === "solid" ? frameBorder.rgb : "none"}
          height={segment.height}
          key={index}
          width={segment.width}
          x={segment.x}
          y={segment.y}
        />
      ))}
    </svg>
  );
}
