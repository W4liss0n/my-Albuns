import { useLayoutEffect, useRef } from "react";

import type { ViewportState } from "../state/viewport";
import type {
  AlbumCanvasMode,
  CanvasMetrics,
} from "./albumCanvasContract";
import type { ContinuousCanvasLayout } from "./canvasGeometry";
import "./CanvasHorizontalScrollbar.css";

interface CanvasHorizontalScrollbarProps {
  centeredSheetId: string | null;
  layout: ContinuousCanvasLayout;
  metrics: CanvasMetrics | null;
  mode: AlbumCanvasMode;
  onCenteredSheetChange(sheetId: string): void;
  onViewportChange(viewport: ViewportState): void;
  viewport: ViewportState;
}

export function CanvasHorizontalScrollbar({
  centeredSheetId,
  layout,
  metrics,
  mode,
  onCenteredSheetChange,
  onViewportChange,
  viewport,
}: CanvasHorizontalScrollbarProps) {
  const scrollbarRef = useRef<HTMLDivElement>(null);
  const bounds =
    metrics && mode.kind === "normal"
      ? layout.offsetBounds(metrics.scale, metrics.width)
      : null;
  const scrollDistance = bounds
    ? Math.max(0, bounds.maximum - bounds.minimum)
    : 0;
  const scrollLeft = bounds
    ? bounds.maximum -
      Math.min(bounds.maximum, Math.max(bounds.minimum, viewport.offsetX))
    : 0;
  const contentWidth = (metrics?.width ?? 0) + scrollDistance;
  const trackWidth = metrics?.width ?? 0;
  const scrollProgress =
    scrollDistance > 0 ? scrollLeft / scrollDistance : 0;
  const idealThumbWidth =
    contentWidth > 0 ? (trackWidth * trackWidth) / contentWidth : 0;
  const thumbWidth = Math.min(
    trackWidth,
    Math.max(24, idealThumbWidth),
  );
  const thumbLeft = scrollProgress * (trackWidth - thumbWidth);

  useLayoutEffect(() => {
    const scrollbar = scrollbarRef.current;
    if (!scrollbar || Math.abs(scrollbar.scrollLeft - scrollLeft) < 0.5) {
      return;
    }
    scrollbar.scrollLeft = scrollLeft;
  }, [scrollLeft]);

  const handleScroll = () => {
    const scrollbar = scrollbarRef.current;
    if (!scrollbar || !bounds || !metrics) return;
    const offsetX = Math.min(
      bounds.maximum,
      Math.max(bounds.minimum, bounds.maximum - scrollbar.scrollLeft),
    );
    if (Math.abs(offsetX - viewport.offsetX) < 0.5) return;

    onViewportChange({ ...viewport, offsetX });
    const nextCenteredSheetId = layout.centeredSheetId(
      offsetX,
      metrics.scale,
      metrics.width,
    );
    if (
      nextCenteredSheetId &&
      nextCenteredSheetId !== centeredSheetId
    ) {
      onCenteredSheetChange(nextCenteredSheetId);
    }
  };

  return (
    <div className="canvas-horizontal-scrollbar-shell">
      <div
        aria-disabled={!bounds}
        aria-label="Navegação horizontal das Lâminas"
        aria-orientation="horizontal"
        aria-valuemax={scrollDistance}
        aria-valuemin={0}
        aria-valuenow={scrollLeft}
        className="canvas-horizontal-scrollbar"
        onScroll={handleScroll}
        ref={scrollbarRef}
        role="scrollbar"
        tabIndex={0}
      >
        <div
          aria-hidden="true"
          className="canvas-horizontal-scrollbar__content"
          style={{ width: `${contentWidth}px` }}
        />
      </div>
      <div
        aria-hidden="true"
        className="canvas-horizontal-scrollbar__visual-track"
      >
        {bounds && trackWidth > 0 ? (
          <span
            className="canvas-horizontal-scrollbar__thumb"
            style={{ left: `${thumbLeft}px`, width: `${thumbWidth}px` }}
          />
        ) : null}
      </div>
    </div>
  );
}
