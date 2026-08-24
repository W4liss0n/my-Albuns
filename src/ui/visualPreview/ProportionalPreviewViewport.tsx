import {
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";

import "./ProportionalPreviewViewport.css";

export interface PreviewOutsideSurfaceAction {
  label: string;
  onFocusChange(focused: boolean): void;
  onPress(): void;
  pressed: boolean;
}

interface ProportionalPreviewViewportProps {
  children: ReactNode;
  height: number;
  label?: string;
  outsideSurfaceAction?: PreviewOutsideSurfaceAction;
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
  outsideSurfaceAction,
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
    <div className="visual-preview-viewport" ref={viewportRef}>
      {outsideSurfaceAction ? (
        <button
          aria-label={outsideSurfaceAction.label}
          aria-pressed={outsideSurfaceAction.pressed}
          className="visual-preview-outside-action"
          onBlur={() => outsideSurfaceAction.onFocusChange(false)}
          onClick={(event) => {
            event.stopPropagation();
            outsideSurfaceAction.onPress();
          }}
          onFocus={(event) =>
            outsideSurfaceAction.onFocusChange(
              event.currentTarget.matches(":focus-visible"),
            )
          }
          type="button"
        />
      ) : null}
      <div
        aria-label={label}
        className="visual-preview-surface"
        onClick={(event) => event.stopPropagation()}
        style={surfaceStyle}
      >
        {children}
      </div>
    </div>
  );
}
