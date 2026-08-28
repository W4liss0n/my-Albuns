import {
  useEffect,
  useRef,
  type CSSProperties,
  type DragEvent as ReactDragEvent,
  type MouseEvent as ReactMouseEvent,
} from "react";

import type { ComposedSheet } from "../domain/project";
import type { ViewportState } from "../state/viewport";
import type { CanvasMetrics } from "./albumCanvasContract";
import {
  CANVAS_VERTICAL_MARGIN_PX,
  type ContinuousCanvasLayout,
} from "./canvasGeometry";
import {
  SHEET_REORDER_INVALID_MESSAGE,
  sheetReorderAutoScrollVelocity,
  type SheetReorderRepresentation,
  type SheetReorderStatus,
} from "./sheetReorderSession";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";
import "./SheetBarReorderOverlay.css";

export interface SheetBarReorderOverlayProps {
  readonly sheets: readonly ComposedSheet[];
  readonly layout: ContinuousCanvasLayout;
  readonly metrics: CanvasMetrics | null;
  readonly viewport: ViewportState;
  readonly representation: SheetReorderRepresentation;
  readonly status: SheetReorderStatus;
  readonly disabled: boolean;
  readonly onPreview: (draggedSheetId: string, targetIndex: number) => void;
  readonly onDrop: () => void;
  readonly onCancel: () => void;
  readonly onNavigate: (sheetId: string) => void;
  readonly onContextMenu: (
    sheetId: string,
    position: { x: number; y: number },
  ) => void;
  readonly onAutoScrollVelocity: (pixelsPerSecond: number) => void;
}

export function SheetBarReorderOverlay(
  props: SheetBarReorderOverlayProps,
) {
  const scale = props.metrics?.scale ?? null;
  const entries =
    scale !== null && scale > 0
      ? props.layout.entriesAtScale(scale)
      : [];
  const sheetById = new Map(
    props.sheets.map((sheet) => [sheet.sheetId, sheet] as const),
  );
  const confirmedIndexById = new Map(
    props.sheets.map((sheet, index) => [sheet.sheetId, index] as const),
  );
  const reorderEnabled = !props.disabled && props.status !== "committing";
  const placeholderEntry =
    props.representation.placeholderIndex === null
      ? null
      : entries[props.representation.placeholderIndex] ?? null;
  const ghostSheetId = props.representation.ghost?.sheetId ?? null;
  const ghostSheet = ghostSheetId ? sheetById.get(ghostSheetId) : undefined;
  const ghostIndex = ghostSheetId
    ? props.representation.order.indexOf(ghostSheetId)
    : -1;
  const ghostEntry = ghostIndex >= 0 ? entries[ghostIndex] ?? null : null;
  const draggedSheetIdRef = useRef<string | null>(null);
  const nativeDragImageCleanupRef = useRef<(() => void) | null>(null);

  useEffect(
    () => () => nativeDragImageCleanupRef.current?.(),
    [],
  );

  useEffect(() => {
    const cancelOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !draggedSheetIdRef.current) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      draggedSheetIdRef.current = null;
      props.onAutoScrollVelocity(0);
      props.onCancel();
    };
    window.addEventListener("keydown", cancelOnEscape, true);
    return () => window.removeEventListener("keydown", cancelOnEscape, true);
  }, [props.onAutoScrollVelocity, props.onCancel]);

  function beginReorder(
    event: ReactDragEvent<HTMLButtonElement>,
    sheetId: string,
    targetIndex: number,
  ) {
    event.stopPropagation();
    if (!reorderEnabled) {
      event.preventDefault();
      return;
    }
    draggedSheetIdRef.current = sheetId;
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", sheetId);
    event.dataTransfer.setData("application/x-myalbuns-sheet", sheetId);
    setNativeDragImage(event, sheetById.get(sheetId));
    props.onPreview(sheetId, targetIndex);
  }

  function setNativeDragImage(
    event: ReactDragEvent<HTMLButtonElement>,
    sheet: ComposedSheet | undefined,
  ) {
    nativeDragImageCleanupRef.current?.();
    if (!sheet) return;

    const bounds = event.currentTarget.getBoundingClientRect();
    const source = event.currentTarget;
    const dragImage = document.createElement("span");
    dragImage.className =
      "sheet-bar-reorder-overlay__native-drag-image";
    dragImage.textContent = `L${String(sheet.number).padStart(2, "0")}`;
    Object.assign(dragImage.style, {
      height: `${bounds.height}px`,
      left: `${bounds.left}px`,
      top: `${bounds.top}px`,
      width: `${bounds.width}px`,
    });
    document.body.append(dragImage);

    const offsetX = dragImageOffset(
      event.clientX - bounds.left,
      bounds.width,
    );
    const offsetY = dragImageOffset(
      event.clientY - bounds.top,
      bounds.height,
    );

    if (typeof event.dataTransfer.setDragImage === "function") {
      try {
        event.dataTransfer.setDragImage(dragImage, offsetX, offsetY);
        scheduleNativeDragFeedbackCleanup(source, dragImage);
        return;
      } catch {
        dragImage.remove();
      }
    } else {
      dragImage.remove();
    }

    source.dataset.nativeDragFallback = "true";
    scheduleNativeDragFeedbackCleanup(source, null);
  }

  function scheduleNativeDragFeedbackCleanup(
    source: HTMLButtonElement,
    dragImage: HTMLElement | null,
  ) {
    let cleanupTimer: number | null = null;
    const cleanup = () => {
      if (cleanupTimer !== null) window.clearTimeout(cleanupTimer);
      dragImage?.remove();
      delete source.dataset.nativeDragFallback;
      if (nativeDragImageCleanupRef.current === cleanup) {
        nativeDragImageCleanupRef.current = null;
      }
    };
    nativeDragImageCleanupRef.current = cleanup;
    cleanupTimer = window.setTimeout(cleanup, 0);
  }

  function previewReorder(
    event: ReactDragEvent<HTMLButtonElement>,
    targetIndex: number,
  ) {
    event.stopPropagation();
    const draggedSheetId = draggedSheetIdRef.current;
    if (!reorderEnabled || !draggedSheetId) return;
    event.preventDefault();
    props.onPreview(draggedSheetId, targetIndex);
  }

  function finishReorder(event: ReactDragEvent<HTMLButtonElement>) {
    event.stopPropagation();
    if (!draggedSheetIdRef.current) return;
    draggedSheetIdRef.current = null;
    props.onAutoScrollVelocity(0);
    props.onCancel();
  }

  function reportAutoScroll(event: ReactDragEvent<HTMLDivElement>) {
    event.stopPropagation();
    if (!reorderEnabled || event.dataTransfer.effectAllowed !== "move") {
      return;
    }
    event.preventDefault();
    event.dataTransfer.dropEffect = "move";
    const bounds = event.currentTarget.getBoundingClientRect();
    props.onAutoScrollVelocity(
      sheetReorderAutoScrollVelocity({
        axis: "horizontal",
        pointerPosition: event.clientX,
        viewportStart: bounds.left,
        viewportEnd: bounds.right,
      }),
    );
  }

  function openContextMenu(
    event: ReactMouseEvent<HTMLButtonElement>,
    sheetId: string,
  ) {
    if (props.disabled) return;
    event.preventDefault();
    event.stopPropagation();
    props.onContextMenu(sheetId, {
      x: event.clientX,
      y: event.clientY,
    });
  }

  return (
    <div
      aria-disabled={!reorderEnabled}
      aria-label="Reordenação pela Barra da Lâmina"
      className="sheet-bar-reorder-overlay"
      data-preview-order={props.representation.order.join(",")}
      data-reorder-state={props.status}
      data-reorder-surface="bar"
      data-sheet-order={props.sheets
        .map((sheet) => sheet.sheetId)
        .join(",")}
      onDragOver={reportAutoScroll}
      onDrop={(event) => {
        event.stopPropagation();
        if (
          !reorderEnabled ||
          event.dataTransfer.effectAllowed !== "move"
        ) {
          return;
        }
        event.preventDefault();
        draggedSheetIdRef.current = null;
        props.onAutoScrollVelocity(0);
        props.onDrop();
      }}
      role="group"
    >
      <span
        aria-hidden="true"
        className="sheet-bar-reorder-overlay__drop-zone"
        data-testid="sheet-reorder-bar-drop-zone"
        style={{
          height: `${SHEET_VISUAL_STYLE.sheetBar.heightPx}px`,
          top: `${CANVAS_VERTICAL_MARGIN_PX}px`,
        }}
      />
      {placeholderEntry && scale !== null ? (
        <span
          aria-hidden="true"
          className="sheet-bar-reorder-overlay__placeholder"
          data-testid="reorder-placeholder"
          style={{
            height: `${SHEET_VISUAL_STYLE.sheetBar.heightPx}px`,
            left: `${placeholderEntry.left * scale + props.viewport.offsetX}px`,
            top: `${CANVAS_VERTICAL_MARGIN_PX}px`,
            width: `${placeholderEntry.width * scale}px`,
          }}
        />
      ) : null}
      {props.representation.order.flatMap((sheetId, index) => {
        const sheet = sheetById.get(sheetId);
        const entry = entries[index];
        if (!sheet || !entry || scale === null) return [];

        const style = {
          height: `${SHEET_VISUAL_STYLE.sheetBar.heightPx}px`,
          left: `${entry.left * scale + props.viewport.offsetX}px`,
          top: `${CANVAS_VERTICAL_MARGIN_PX}px`,
          width: `${entry.width * scale}px`,
        } satisfies CSSProperties;
        return [
          <button
            aria-label={`Reordenar Lâmina ${String(sheet.number).padStart(2, "0")} pela Barra`}
            className="sheet-bar-reorder-overlay__handle"
            data-reorder-ghost={ghostSheetId === sheet.sheetId || undefined}
            data-reorder-shift={
              confirmedIndexById.get(sheet.sheetId) !== index || undefined
            }
            data-sheet-id={sheet.sheetId}
            data-slot-index={index}
            draggable={reorderEnabled}
            key={sheet.sheetId}
            onContextMenu={(event) =>
              openContextMenu(event, sheet.sheetId)
            }
            onClick={() => props.onNavigate(sheet.sheetId)}
            onDragEnd={finishReorder}
            onDragEnter={(event) => previewReorder(event, index)}
            onDragStart={(event) =>
              beginReorder(event, sheet.sheetId, index)
            }
            style={style}
            type="button"
          >
            <span
              aria-hidden="true"
              className="sheet-bar-reorder-overlay__handle-label"
            >
              L{String(sheet.number).padStart(2, "0")}
            </span>
          </button>,
        ];
      })}
      {ghostSheet && ghostEntry && scale !== null ? (
        <span
          aria-hidden="true"
          className="sheet-bar-reorder-overlay__ghost"
          data-sheet-id={ghostSheet.sheetId}
          data-testid="reorder-ghost"
          style={{
            height: `${SHEET_VISUAL_STYLE.sheetBar.heightPx}px`,
            left: `${ghostEntry.left * scale + props.viewport.offsetX}px`,
            top: `${CANVAS_VERTICAL_MARGIN_PX}px`,
            width: `${ghostEntry.width * scale}px`,
          }}
        >
          {String(ghostSheet.number).padStart(2, "0")}
        </span>
      ) : null}
      {props.status === "invalid" && props.representation.ghost ? (
        <span
          className="sheet-bar-reorder-overlay__invalid"
          data-reorder-invalid-indicator
          role="status"
        >
          {SHEET_REORDER_INVALID_MESSAGE}
        </span>
      ) : null}
    </div>
  );
}

function dragImageOffset(pointerOffset: number, extent: number): number {
  if (!Number.isFinite(pointerOffset) || !Number.isFinite(extent)) return 0;
  return Math.round(Math.min(Math.max(pointerOffset, 0), Math.max(extent, 0)));
}
