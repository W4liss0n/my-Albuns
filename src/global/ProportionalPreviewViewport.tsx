import {
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";

import "./ProportionalPreviewViewport.css";

interface ProportionalPreviewViewportProps {
  children: ReactNode;
  height: number;
  label?: string;
  width: number;
}

interface PreviewSurfaceSize {
  height: number;
  sourceHeight: number;
  sourceWidth: number;
  width: number;
}

export function ProportionalPreviewViewport({
  children,
  height,
  label,
  width,
}: ProportionalPreviewViewportProps) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const safeWidth = Math.max(1, width);
  const safeHeight = Math.max(1, height);
  const [surfaceSize, setSurfaceSize] =
    useState<PreviewSurfaceSize | null>(null);

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;

    const fitSurface = (availableWidth: number, availableHeight: number) => {
      if (availableWidth <= 0 || availableHeight <= 0) return;

      const scale = Math.min(
        availableWidth / safeWidth,
        availableHeight / safeHeight,
      );
      const nextSize = {
        height: safeHeight * scale,
        sourceHeight: safeHeight,
        sourceWidth: safeWidth,
        width: safeWidth * scale,
      };

      setSurfaceSize((currentSize) =>
        currentSize &&
        currentSize.sourceWidth === nextSize.sourceWidth &&
        currentSize.sourceHeight === nextSize.sourceHeight &&
        Math.abs(currentSize.width - nextSize.width) < 0.25 &&
        Math.abs(currentSize.height - nextSize.height) < 0.25
          ? currentSize
          : nextSize,
      );
    };

    const initialBounds = viewport.getBoundingClientRect();
    fitSurface(initialBounds.width, initialBounds.height);

    if (typeof ResizeObserver === "undefined") return;

    const observer = new ResizeObserver(([entry]) => {
      fitSurface(entry.contentRect.width, entry.contentRect.height);
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [safeHeight, safeWidth]);

  const fittedSurface =
    surfaceSize?.sourceWidth === safeWidth &&
    surfaceSize.sourceHeight === safeHeight
      ? surfaceSize
      : null;
  const surfaceStyle: CSSProperties = {
    aspectRatio: `${safeWidth} / ${safeHeight}`,
    height: fittedSurface?.height,
    maxHeight: "100%",
    maxWidth: "100%",
    width: fittedSurface?.width ?? "100%",
  };

  return (
    <div className="new-project-proportional-preview-viewport" ref={viewportRef}>
      <div
        aria-label={label}
        className="new-project-proportional-preview-surface"
        style={surfaceStyle}
      >
        {children}
      </div>
    </div>
  );
}
