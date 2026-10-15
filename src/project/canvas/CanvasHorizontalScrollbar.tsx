import { useLayoutEffect, useRef } from "react";

import type { ViewportState } from "../../state/viewport";
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

  // Reading or writing scrollLeft forces a layout. Wheel steps arrive faster
  // than frames on a slow PC, so the native scroller follows the viewport once
  // per frame, when the frame lays out anyway.
  const targetScrollLeftRef = useRef(scrollLeft);
  const writtenScrollLeftRef = useRef<number | null>(null);
  const writeFrameRef = useRef(0);
  useLayoutEffect(() => {
    targetScrollLeftRef.current = scrollLeft;
    if (writeFrameRef.current !== 0) return;
    writeFrameRef.current = requestAnimationFrame(() => {
      writeFrameRef.current = 0;
      const scrollbar = scrollbarRef.current;
      if (!scrollbar) return;
      const target = targetScrollLeftRef.current;
      writtenScrollLeftRef.current = target;
      if (Math.abs(scrollbar.scrollLeft - target) >= 0.5) {
        scrollbar.scrollLeft = target;
      }
    });
  }, [scrollLeft]);
  useLayoutEffect(() => () => {
    cancelAnimationFrame(writeFrameRef.current);
    writeFrameRef.current = 0;
  }, []);

  const handleScroll = () => {
    const scrollbar = scrollbarRef.current;
    if (!scrollbar || !bounds || !metrics) return;
    // The echo of a write above may arrive after newer wheel steps; it must
    // not pull the Canvas back to where the scroller was.
    const written = writtenScrollLeftRef.current;
    if (written !== null && Math.abs(scrollbar.scrollLeft - written) < 0.5) {
      return;
    }
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
        aria-label="Navegação horizontal das lâminas"
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
