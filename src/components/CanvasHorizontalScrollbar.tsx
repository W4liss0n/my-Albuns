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
  const entries = layout.entriesAtScale(metrics?.scale ?? 1);
  const centeredIndex = entries.findIndex(
    (entry) => entry.sheetId === centeredSheetId,
  );
  const navigationEnabled =
    mode.kind === "normal" && metrics !== null && centeredIndex >= 0;
  const previousSheetId = navigationEnabled
    ? entries[centeredIndex - 1]?.sheetId ?? null
    : null;
  const nextSheetId = navigationEnabled
    ? entries[centeredIndex + 1]?.sheetId ?? null
    : null;

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

  const centerAdjacentSheet = (sheetId: string | null) => {
    if (!sheetId || !metrics || mode.kind !== "normal") return;
    const offsetX = layout.centeredOffset(
      sheetId,
      metrics.scale,
      metrics.width,
    );
    if (offsetX === null) return;
    onViewportChange({ ...viewport, offsetX });
    onCenteredSheetChange(sheetId);
  };

  return (
    <div className="canvas-horizontal-scrollbar-shell">
      <button
        aria-label="Lâmina anterior"
        className="canvas-sheet-navigation canvas-sheet-navigation--previous"
        disabled={previousSheetId === null}
        type="button"
        onClick={() => centerAdjacentSheet(previousSheetId)}
      >
        <span aria-hidden="true">‹</span>
      </button>
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
      <button
        aria-label="Próxima Lâmina"
        className="canvas-sheet-navigation canvas-sheet-navigation--next"
        disabled={nextSheetId === null}
        type="button"
        onClick={() => centerAdjacentSheet(nextSheetId)}
      >
        <span aria-hidden="true">›</span>
      </button>
    </div>
  );
}
