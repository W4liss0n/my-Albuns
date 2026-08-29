import {
  useRef,
  type CSSProperties,
  type MouseEvent as ReactMouseEvent,
} from "react";

import type { ComposedSheet } from "../domain/project";
import type { ViewportState } from "../state/viewport";
import type { CanvasMetrics } from "./albumCanvasContract";
import {
  CANVAS_VERTICAL_MARGIN_PX,
  createCanvasSheetPresentation,
  type ContinuousCanvasLayout,
} from "./canvasGeometry";
import { createCanvasSheetViewGeometry } from "./canvasSheetViewGeometry";
import {
  SHEET_REORDER_INVALID_MESSAGE,
  sheetReorderAutoScrollVelocity,
  type SheetReorderRepresentation,
  type SheetReorderStatus,
} from "./sheetReorderSession";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";
import {
  useSheetPointerReorder,
  type SheetReorderPointerPosition,
} from "./useSheetPointerReorder";
import "./SheetBarReorderOverlay.css";

export interface SheetBarReorderOverlayProps {
  readonly sheets: readonly ComposedSheet[];
  readonly layout: ContinuousCanvasLayout;
  readonly metrics: CanvasMetrics | null;
  readonly bleedUm?: number;
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
  const firstSheetBounds =
    scale === null || !props.sheets[0]
      ? null
      : visibleSheetBounds(
          props.sheets[0],
          scale,
          props.bleedUm,
        );
  const overlayRef = useRef<HTMLDivElement | null>(null);
  const ghostAnchorRef = useRef<GhostAnchor | null>(null);
  const pointerReorder = useSheetPointerReorder({
    enabled: reorderEnabled,
    onActivate: props.onNavigate,
    onCancel: props.onCancel,
    onDrop: props.onDrop,
    onFinish: () => props.onAutoScrollVelocity(0),
    onMove: reportAutoScroll,
    onPreview: props.onPreview,
    resolveTarget: ({ clientX }) =>
      scale === null
        ? null
        : resolveHorizontalTarget(
            entries,
            scale,
            props.viewport.offsetX,
            overlayRef.current?.getBoundingClientRect().left ?? 0,
            clientX,
          ),
    targetGeometryRevision: props.viewport.offsetX,
    validRelease: (position) =>
      pointInsideRenderedElement(overlayRef.current, position),
  });
  const ghostSheetId =
    pointerReorder.pointer?.sourceId ??
    props.representation.ghost?.sheetId ??
    null;
  const ghostSheet = ghostSheetId ? sheetById.get(ghostSheetId) : undefined;
  const ghostIndex = ghostSheetId
    ? props.representation.order.indexOf(ghostSheetId)
    : -1;
  const ghostEntry = ghostIndex >= 0 ? entries[ghostIndex] ?? null : null;
  const placeholderBounds =
    scale === null || !ghostSheet
      ? null
      : visibleSheetBounds(ghostSheet, scale, props.bleedUm);
  const ghostBounds = placeholderBounds;

  function reportAutoScroll(position: SheetReorderPointerPosition) {
    const bounds = overlayRef.current?.getBoundingClientRect();
    if (!bounds) return;
    props.onAutoScrollVelocity(
      sheetReorderAutoScrollVelocity({
        axis: "horizontal",
        pointerPosition: position.clientX,
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
      onLostPointerCapture={pointerReorder.lostCapture}
      onPointerCancel={pointerReorder.cancel}
      onPointerMove={pointerReorder.move}
      onPointerUp={pointerReorder.end}
      ref={overlayRef}
      role="group"
    >
      <span
        aria-hidden="true"
        className="sheet-bar-reorder-overlay__drop-zone"
        data-testid="sheet-reorder-bar-drop-zone"
        style={{
          height: `${SHEET_VISUAL_STYLE.sheetBar.heightPx}px`,
          top: `${firstSheetBounds?.top ?? CANVAS_VERTICAL_MARGIN_PX}px`,
        }}
      />
      {placeholderEntry && placeholderBounds && scale !== null ? (
        <span
          aria-hidden="true"
          className="sheet-bar-reorder-overlay__placeholder"
          data-testid="reorder-placeholder"
          style={{
            height: `${placeholderBounds.height}px`,
            left: `${placeholderEntry.left * scale + props.viewport.offsetX}px`,
            top: `${placeholderBounds.top}px`,
            width: `${placeholderEntry.width * scale}px`,
          }}
        />
      ) : null}
      {props.representation.order.flatMap((sheetId, index) => {
        const sheet = sheetById.get(sheetId);
        const entry = entries[index];
        if (!sheet || !entry || scale === null) return [];

        const fullWidth = entry.width * scale;
        const sheetBounds = visibleSheetBounds(
          sheet,
          scale,
          props.bleedUm,
        );
        const freeRegionStart = Math.min(
          fullWidth,
          Math.max(
            SHEET_VISUAL_STYLE.sheetBar.swapActionCenterPx +
              SHEET_VISUAL_STYLE.sheetBar.actionSizePx / 2,
            fullWidth / 2 +
              SHEET_VISUAL_STYLE.sheetBar.actionSizePx / 2,
          ),
        );
        const style = {
          height: `${SHEET_VISUAL_STYLE.sheetBar.heightPx}px`,
          left: `${entry.left * scale + props.viewport.offsetX + freeRegionStart}px`,
          top: `${sheetBounds.top}px`,
          width: `${Math.max(0, fullWidth - freeRegionStart)}px`,
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
            key={sheet.sheetId}
            onContextMenu={(event) =>
              openContextMenu(event, sheet.sheetId)
            }
            onClick={(event) => {
              if (
                pointerReorder.consumeClickSuppression(sheet.sheetId)
              ) {
                event.preventDefault();
                event.stopPropagation();
                return;
              }
              props.onNavigate(sheet.sheetId);
            }}
            onPointerDown={(event) => {
              const overlayBounds =
                overlayRef.current?.getBoundingClientRect();
              const fullLeft =
                entry.left * scale + props.viewport.offsetX;
              ghostAnchorRef.current = {
                height: sheetBounds.height,
                offsetX:
                  event.clientX -
                  (overlayBounds?.left ?? 0) -
                  fullLeft,
                offsetY:
                  event.clientY -
                  (overlayBounds?.top ?? 0) -
                  sheetBounds.top,
                overlayLeft: overlayBounds?.left ?? 0,
                overlayTop: overlayBounds?.top ?? 0,
                width: fullWidth,
              };
              pointerReorder.begin(
                event,
                sheet.sheetId,
                overlayRef.current,
              );
            }}
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
      {ghostSheet && ghostEntry && ghostBounds && scale !== null ? (
        <span
          aria-hidden="true"
          className="sheet-bar-reorder-overlay__ghost"
          data-pointer-x={pointerReorder.pointer?.clientX}
          data-pointer-y={pointerReorder.pointer?.clientY}
          data-sheet-id={ghostSheet.sheetId}
          data-testid="reorder-ghost"
          style={ghostStyle(
            pointerReorder.pointer,
            ghostAnchorRef.current,
            {
              height: ghostBounds.height,
              left: ghostEntry.left * scale + props.viewport.offsetX,
              top: ghostBounds.top,
              width: ghostEntry.width * scale,
            },
          )}
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

interface GhostAnchor {
  readonly height: number;
  readonly offsetX: number;
  readonly offsetY: number;
  readonly overlayLeft: number;
  readonly overlayTop: number;
  readonly width: number;
}

function ghostStyle(
  pointer: ReturnType<typeof useSheetPointerReorder>["pointer"],
  anchor: GhostAnchor | null,
  fallback: { height: number; left: number; top: number; width: number },
): CSSProperties {
  if (!pointer || !anchor) {
    return {
      height: `${fallback.height}px`,
      left: `${fallback.left}px`,
      top: `${fallback.top}px`,
      width: `${fallback.width}px`,
    };
  }
  return {
    height: `${anchor.height}px`,
    left: `${pointer.clientX - anchor.overlayLeft - anchor.offsetX}px`,
    top: `${pointer.clientY - anchor.overlayTop - anchor.offsetY}px`,
    width: `${anchor.width}px`,
  };
}

function resolveHorizontalTarget(
  entries: readonly { readonly left: number; readonly width: number }[],
  scale: number,
  viewportOffsetX: number,
  overlayLeft: number,
  clientX: number,
): number | null {
  if (entries.length === 0) return null;
  const localX = clientX - overlayLeft;
  let nearestIndex = 0;
  let nearestDistance = Number.POSITIVE_INFINITY;
  entries.forEach((entry, index) => {
    const left = entry.left * scale + viewportOffsetX;
    const right = left + entry.width * scale;
    if (localX >= left && localX <= right) {
      nearestIndex = index;
      nearestDistance = 0;
      return;
    }
    const distance = Math.min(Math.abs(localX - left), Math.abs(localX - right));
    if (distance < nearestDistance) {
      nearestIndex = index;
      nearestDistance = distance;
    }
  });
  return nearestIndex;
}

function pointInsideRenderedElement(
  element: HTMLElement | null,
  position: SheetReorderPointerPosition,
): boolean {
  if (!element) return false;
  const bounds = element.getBoundingClientRect();
  if (bounds.width === 0 && bounds.height === 0) return true;
  return (
    position.clientX >= bounds.left &&
    position.clientX <= bounds.right &&
    position.clientY >= bounds.top &&
    position.clientY <= bounds.bottom
  );
}

function visibleSheetBounds(
  sheet: ComposedSheet,
  scale: number,
  bleedUm: number | undefined,
) {
  const geometry = createCanvasSheetViewGeometry(
    sheet,
    createCanvasSheetPresentation(sheet),
    bleedUm,
    true,
  ).visibleOuterBounds;
  return {
    height: geometry.height * scale,
    top: CANVAS_VERTICAL_MARGIN_PX + geometry.y * scale,
  };
}
