import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type DragEvent as ReactDragEvent,
  type MouseEvent as ReactMouseEvent,
  type ReactNode,
} from "react";
import { Button } from "react-aria-components";
import { ChevronDown, ChevronRight, PanelsTopLeft } from "lucide-react";

import type {
  AlbumInformation,
  AlbumInformationImpact,
  AlbumInformationValidation,
  ComposedPhoto,
  ComposedSheet,
  DisplayUnit,
  DocumentSnapshot,
  FrameSnapshot,
  MediaCatalogItem,
  ProjectedFrameBorder,
  ProjectedVisualDefaults,
  SheetSnapshot,
} from "../domain/project";
import type {
  AlbumDesignProjectDraft,
  AlbumInformationProjectDraft,
} from "../application/projectSettingsDraft";
import type { MediaPreview } from "../application/projectPorts";
import { renderableMediaPreviewUrls } from "../application/mediaPreviews";
import { ActionButton, AppIcon, EmptyState } from "../ui";
import { AlbumDesignForm } from "./AlbumDesignForm";
import { AlbumInformationForm } from "./AlbumInformationForm";
import { SheetPreview } from "./SheetPreview";
import {
  SheetDesignInspector,
  type SheetDesignScope,
} from "./SheetDesignInspector";
import { inactiveSideCssGradient } from "./sheetVisualStyle";
import {
  SHEET_REORDER_INVALID_MESSAGE,
  sheetReorderAutoScrollVelocity,
  type SheetReorderRepresentation,
  type SheetReorderStatus,
} from "./sheetReorderSession";
import "./InspectorPanel.css";

const PHOTO_ZOOM_KEYS = new Set([
  "ArrowLeft",
  "ArrowRight",
  "ArrowUp",
  "ArrowDown",
  "Home",
  "End",
  "PageUp",
  "PageDown",
]);

const ALBUM_INFORMATION_FORM_ID = "album-information-settings";
const ALBUM_DESIGN_FORM_ID = "album-design-settings";

export type InspectorContext =
  | { kind: "album" }
  | { kind: "sheet"; sheet: ComposedSheet }
  | {
      kind: "frame";
      frame: FrameSnapshot;
      composedPhoto: ComposedPhoto | null;
      editingSheet?: ComposedSheet;
    };

export type InspectorSectionState =
  | {
      kind: "controlled";
      values: Readonly<Record<string, boolean>>;
      onChange(preferenceKey: string, open: boolean): void;
    }
  | { kind: "local" };

export interface InspectorPanelProps {
  context: InspectorContext;
  displayedPhotoZoom: number;
  displayedPhotoPanX: number;
  zoomCommitting: boolean;
  document: DocumentSnapshot;
  presentationUnit: DisplayUnit;
  mediaItems: readonly MediaCatalogItem[];
  sheetStates: readonly SheetSnapshot[];
  sheets: readonly ComposedSheet[];
  frameBorder: ProjectedFrameBorder;
  visualDefaults: ProjectedVisualDefaults;
  focusedSheetId: string | null;
  mediaPreviews: Readonly<Record<string, MediaPreview>>;
  revision: number;
  onBeginPhotoZoom(): void;
  onUpdatePhotoZoom(value: number): void;
  onFinishPhotoZoom(): void | Promise<void>;
  onApplyAlbumInformation(
    draft: AlbumInformationProjectDraft,
    impact: AlbumInformationImpact,
  ): Promise<boolean>;
  onApplyAlbumDesign(
    draft: AlbumDesignProjectDraft,
  ): Promise<boolean>;
  onValidateAlbumInformation(
    information: AlbumInformation,
  ): Promise<AlbumInformationValidation>;
  onPresentationUnitChange(unit: DisplayUnit | null): void;
  onNavigateToSheet(sheetId: string): void;
  onOpenSheetContextMenu?(
    sheetId: string,
    position: { x: number; y: number },
  ): void;
  sheetReorder?: {
    disabled: boolean;
    representation: SheetReorderRepresentation;
    status: SheetReorderStatus;
    onPreview(draggedSheetId: string, targetIndex: number): void;
    onDrop(): void;
    onCancel(): void;
  };
  sectionState: InspectorSectionState;
}

export function InspectorPanel({
  context,
  displayedPhotoZoom,
  displayedPhotoPanX,
  zoomCommitting,
  document,
  presentationUnit,
  mediaItems,
  sheetStates,
  sheets,
  frameBorder,
  visualDefaults,
  focusedSheetId,
  mediaPreviews,
  revision,
  onBeginPhotoZoom,
  onUpdatePhotoZoom,
  onFinishPhotoZoom,
  onApplyAlbumInformation,
  onApplyAlbumDesign,
  onPresentationUnitChange,
  onValidateAlbumInformation,
  onNavigateToSheet,
  onOpenSheetContextMenu,
  sheetReorder,
  sectionState,
}: InspectorPanelProps) {
  const mediaPreviewUrls = useMemo(
    () => renderableMediaPreviewUrls(mediaPreviews),
    [mediaPreviews],
  );
  const [informationDirty, setInformationDirty] = useState(false);
  const [designDirty, setDesignDirty] = useState(false);
  const [sheetScopeSelection, setSheetScopeSelection] = useState<{
    sheetId: string;
    scope: SheetDesignScope;
  } | null>(null);
  const draggedSheetIdRef = useRef<string | null>(null);
  const gridAutoScrollRef = useRef<{
    frameId: number | null;
    lastTimestamp: number | null;
    velocity: number;
    viewport: HTMLElement | null;
  }>({
    frameId: null,
    lastTimestamp: null,
    velocity: 0,
    viewport: null,
  });
  const sheetStateById = new Map(
    sheetStates.map((sheet) => [sheet.id, sheet] as const),
  );
  const composedSheetById = new Map(
    sheets.map((sheet) => [sheet.sheetId, sheet] as const),
  );
  const sheetReorderEnabled =
    sheetReorder !== undefined &&
    !sheetReorder.disabled &&
    sheetReorder.status !== "committing";
  const orderedSheets = sheetReorder
    ? sheetReorder.representation.order.flatMap((sheetId) => {
        const sheet = composedSheetById.get(sheetId);
        return sheet ? [sheet] : [];
      })
    : sheets;

  function stopGridAutoScroll() {
    const state = gridAutoScrollRef.current;
    if (state.frameId !== null) {
      window.cancelAnimationFrame(state.frameId);
    }
    state.frameId = null;
    state.lastTimestamp = null;
    state.velocity = 0;
    state.viewport = null;
  }

  function advanceGridAutoScroll(timestamp: number) {
    const state = gridAutoScrollRef.current;
    state.frameId = null;
    if (!state.viewport || state.velocity === 0) {
      state.lastTimestamp = null;
      return;
    }

    const previousTimestamp = state.lastTimestamp;
    state.lastTimestamp = timestamp;
    if (previousTimestamp !== null) {
      const elapsedMs = Math.min(
        50,
        Math.max(0, timestamp - previousTimestamp),
      );
      state.viewport.scrollTop += (state.velocity * elapsedMs) / 1_000;
    }
    state.frameId = window.requestAnimationFrame(advanceGridAutoScroll);
  }

  function updateGridAutoScroll(
    viewport: HTMLElement,
    velocity: number,
  ) {
    if (velocity === 0) {
      stopGridAutoScroll();
      return;
    }
    const state = gridAutoScrollRef.current;
    if (state.viewport !== viewport) state.lastTimestamp = null;
    state.viewport = viewport;
    state.velocity = velocity;
    if (state.frameId === null) {
      state.frameId = window.requestAnimationFrame(advanceGridAutoScroll);
    }
  }

  useEffect(() => () => stopGridAutoScroll(), []);

  useEffect(() => {
    const cancelGridReorderOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !draggedSheetIdRef.current) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      stopGridAutoScroll();
      draggedSheetIdRef.current = null;
      sheetReorder?.onCancel();
    };
    window.addEventListener("keydown", cancelGridReorderOnEscape, true);
    return () =>
      window.removeEventListener("keydown", cancelGridReorderOnEscape, true);
  }, [sheetReorder]);

  function beginGridReorder(
    event: ReactDragEvent<HTMLDivElement>,
    sheetId: string,
    targetIndex: number,
  ) {
    if (!sheetReorder || !sheetReorderEnabled) {
      event.preventDefault();
      return;
    }
    draggedSheetIdRef.current = sheetId;
    event.dataTransfer.effectAllowed = "move";
    event.dataTransfer.setData("text/plain", sheetId);
    sheetReorder.onPreview(sheetId, targetIndex);
  }

  function previewGridReorder(
    event: ReactDragEvent<HTMLDivElement>,
    targetIndex: number,
  ) {
    const draggedSheetId = draggedSheetIdRef.current;
    if (!sheetReorder || !sheetReorderEnabled || !draggedSheetId) return;
    event.preventDefault();
    sheetReorder.onPreview(draggedSheetId, targetIndex);
  }

  function autoScrollGrid(event: ReactDragEvent<HTMLDivElement>) {
    if (
      !sheetReorder ||
      !sheetReorderEnabled ||
      !draggedSheetIdRef.current
    ) {
      return;
    }
    event.preventDefault();
    const viewport = event.currentTarget.closest<HTMLElement>(
      ".inspector-scroll",
    );
    if (!viewport) return;
    const bounds = viewport.getBoundingClientRect();
    const velocity = sheetReorderAutoScrollVelocity({
      axis: "vertical",
      pointerPosition: event.clientY,
      viewportStart: bounds.top,
      viewportEnd: bounds.bottom,
    });
    updateGridAutoScroll(viewport, velocity);
  }

  function openGridContextMenu(
    event: ReactMouseEvent<HTMLDivElement>,
    sheetId: string,
  ) {
    if (!onOpenSheetContextMenu) return;
    event.preventDefault();
    onOpenSheetContextMenu(sheetId, { x: event.clientX, y: event.clientY });
  }
  const editingSheet =
    context.kind === "sheet"
      ? context.sheet
      : context.kind === "frame"
        ? context.editingSheet ?? null
        : null;
  const selectedSheetScope = editingSheet
    ? normalizeSheetScope(
        sheetScopeSelection?.sheetId === editingSheet.sheetId
          ? sheetScopeSelection.scope
          : defaultSheetScope(editingSheet),
        editingSheet,
      )
    : null;

  useEffect(() => {
    if (!editingSheet) setSheetScopeSelection(null);
  }, [editingSheet]);

  return (
    <aside
      id="contextual-panel"
      className="inspector"
      aria-label="Painel contextual"
    >
      <div className="inspector-scroll">
        {context.kind === "frame" ? (
          <>
            <div className="context-heading">
              <span>Frame selecionado</span>
              <h2>{context.composedPhoto?.name ?? "Frame placeholder"}</h2>
            </div>
            <InspectorSection
              key="frame-photo-design"
              title="Design"
              preferenceKey="frame-photo.design"
              sectionState={sectionState}
              defaultOpen
            >
              <PropertyRow
                label="Frame"
                value={context.frame.id.replace("frame-", "").toUpperCase()}
              />
              <PropertyRow
                label="Pan horizontal"
                value={`${Math.round(displayedPhotoPanX * 100)}%`}
              />
              {context.frame.photo && context.composedPhoto && (
                <label className="photo-zoom-control">
                  <span className="photo-zoom-label">
                    <span>Zoom da Foto</span>
                    <output>{Math.round(displayedPhotoZoom * 100)}%</output>
                  </span>
                  <input
                    className="ui-range"
                    type="range"
                    aria-label="Zoom da Foto"
                    min={
                      context.composedPhoto.placement.zoomRange.minimum * 100
                    }
                    max={
                      context.composedPhoto.placement.zoomRange.maximum * 100
                    }
                    step="1"
                    value={Math.round(displayedPhotoZoom * 100)}
                    disabled={zoomCommitting}
                    onPointerDown={onBeginPhotoZoom}
                    onChange={(event) =>
                      onUpdatePhotoZoom(
                        Number(event.currentTarget.value) / 100,
                      )
                    }
                    onPointerUp={() => void onFinishPhotoZoom()}
                    onKeyDown={(event) => {
                      if (PHOTO_ZOOM_KEYS.has(event.key)) {
                        onBeginPhotoZoom();
                      }
                    }}
                    onKeyUp={(event) => {
                      if (PHOTO_ZOOM_KEYS.has(event.key)) {
                        void onFinishPhotoZoom();
                      }
                    }}
                    onBlur={() => void onFinishPhotoZoom()}
                  />
                </label>
              )}
            </InspectorSection>
          </>
        ) : context.kind === "sheet" && selectedSheetScope ? (
          <InspectorSection
            accessibleTitle="Design da Lâmina"
            key="sheet-design"
            title="Design da Lâmina"
            preferenceKey="sheet.design"
            sectionState={sectionState}
            defaultOpen
          >
            <SheetDesignInspector
              frameBorder={frameBorder}
              mediaPreviewUrls={mediaPreviewUrls}
              scope={selectedSheetScope}
              sheet={context.sheet}
              onScopeChange={(scope) =>
                setSheetScopeSelection({
                  sheetId: context.sheet.sheetId,
                  scope,
                })
              }
            />
          </InspectorSection>
        ) : (
          <>
            <InspectorSection
              action={
                <ActionButton
                  density="compact"
                  disabled={!informationDirty}
                  form={ALBUM_INFORMATION_FORM_ID}
                  type="submit"
                  variant={informationDirty ? "primary" : "quiet"}
                >
                  Aplicar
                </ActionButton>
              }
              accessibleTitle="Informações do Álbum"
              key="album-information"
              title="Informações do Álbum"
              preferenceKey="album.information"
              sectionState={sectionState}
              defaultOpen
            >
              <div className="inspector-subsections">
                <AlbumInformationForm
                  document={document}
                  formId={ALBUM_INFORMATION_FORM_ID}
                  revision={revision}
                  onApply={onApplyAlbumInformation}
                  onPresentationUnitChange={onPresentationUnitChange}
                  onReadyChange={setInformationDirty}
                  onValidate={onValidateAlbumInformation}
                  sheetStates={sheetStates}
                />
              </div>
            </InspectorSection>
            <InspectorSection
              action={
                <ActionButton
                  density="compact"
                  disabled={!designDirty}
                  form={ALBUM_DESIGN_FORM_ID}
                  type="submit"
                  variant={designDirty ? "primary" : "quiet"}
                >
                  Aplicar
                </ActionButton>
              }
              accessibleTitle="Design do Álbum"
              key="album-design"
              title="Design do Álbum"
              preferenceKey="album.design"
              sectionState={sectionState}
              defaultOpen
            >
              <AlbumDesignForm
                document={document}
                presentationUnit={presentationUnit}
                formId={ALBUM_DESIGN_FORM_ID}
                mediaItems={mediaItems}
                mediaPreviews={mediaPreviews}
                revision={revision}
                value={visualDefaults}
                onApply={onApplyAlbumDesign}
                onReadyChange={setDesignDirty}
              />
            </InspectorSection>
            <InspectorSection
              accessibleTitle="Grade de Lâminas"
              key="album-sheet-grid"
              title="Grade de Lâminas"
              preferenceKey="album.sheet-grid"
              sectionState={sectionState}
              meta={sheets.length}
              defaultOpen
            >
              {sheets.length === 0 ? (
                <EmptyState
                  density="compact"
                  description="As Lâminas do Projeto aparecerão aqui."
                  icon={<AppIcon icon={PanelsTopLeft} size={16} />}
                  title="Nenhuma Lâmina na Grade"
                />
              ) : (
                <div
                  className="sheet-grid"
                  data-reorder-state={sheetReorder?.status ?? "idle"}
                  data-reorder-surface="grid"
                  data-sheet-order={sheets.map((sheet) => sheet.sheetId).join(",")}
                  data-testid="sheet-reorder-grid"
                  onDragOver={autoScrollGrid}
                  onDrop={(event) => {
                    stopGridAutoScroll();
                    if (!sheetReorder || !sheetReorderEnabled) return;
                    event.preventDefault();
                    draggedSheetIdRef.current = null;
                    sheetReorder.onDrop();
                  }}
                >
                  {orderedSheets.map((sheet, index) => {
                    const number = String(sheet.number).padStart(2, "0");
                    const pageMetadata = formatSheetPageMetadata(
                      sheetStateById.get(sheet.sheetId),
                    );
                    const visualPageLabel =
                      pageMetadata?.visualLabel ?? `Lâmina ${number}`;
                    const accessiblePageLabel =
                      pageMetadata?.accessibleLabel ?? `Lâmina ${number}`;
                    const active = sheet.sheetId === focusedSheetId;
                    const tileStyle = {
                      aspectRatio: `${document.sheetWidthUm} / ${document.sheetHeightUm}`,
                      ...(sheet.activeSides === "both"
                        ? {}
                        : {
                            "--sheet-inactive-side-gradient":
                              inactiveSideCssGradient(sheet.activeSides),
                          }),
                    } as CSSProperties;
                    const reorderGhost =
                      sheetReorder?.representation.ghost?.sheetId ===
                      sheet.sheetId;
                    return (
                      <div
                        className="sheet-grid-slot"
                        data-reorder-ghost={reorderGhost || undefined}
                        data-sheet-id={sheet.sheetId}
                        draggable={sheetReorderEnabled}
                        key={sheet.sheetId}
                        onContextMenu={(event) =>
                          openGridContextMenu(event, sheet.sheetId)
                        }
                        onDragEnd={() => {
                          if (!draggedSheetIdRef.current) return;
                          stopGridAutoScroll();
                          draggedSheetIdRef.current = null;
                          sheetReorder?.onCancel();
                        }}
                        onDragEnter={(event) =>
                          previewGridReorder(event, index)
                        }
                        onDragStart={(event) =>
                          beginGridReorder(event, sheet.sheetId, index)
                        }
                      >
                        {sheetReorder?.representation.placeholderIndex ===
                        index ? (
                          <span
                            aria-hidden="true"
                            className="sheet-reorder-placeholder"
                            data-testid="reorder-placeholder"
                          />
                        ) : null}
                      <Button
                        aria-current={active ? "true" : undefined}
                        aria-label={`Ir para Lâmina ${number}, ${accessiblePageLabel}`}
                        className={active ? "sheet-tile active" : "sheet-tile"}
                        data-active-sides={sheet.activeSides}
                        style={tileStyle}
                        onPress={() => onNavigateToSheet(sheet.sheetId)}
                      >
                        <SheetPreview
                          frameBorder={frameBorder}
                          sheet={sheet}
                          mediaPreviewUrls={mediaPreviewUrls}
                        />
                        <span aria-hidden="true" className="sheet-tile__number">
                          {number}
                        </span>
                        <span aria-hidden="true" className="sheet-tile__pages">
                          {visualPageLabel}
                        </span>
                      </Button>
                      </div>
                    );
                  })}
                  {sheetReorder?.representation.ghost ? (
                    <span
                      aria-hidden="true"
                      className="sheet-reorder-ghost"
                      data-testid="reorder-ghost"
                    >
                      {sheetStateById.get(
                        sheetReorder.representation.ghost.sheetId,
                      )?.number ?? ""}
                    </span>
                  ) : null}
                  {sheetReorder?.status === "invalid" &&
                  sheetReorder.representation.ghost ? (
                    <span
                      className="sheet-grid__reorder-invalid"
                      data-reorder-invalid-indicator
                      role="status"
                    >
                      {SHEET_REORDER_INVALID_MESSAGE}
                    </span>
                  ) : null}
                </div>
              )}
            </InspectorSection>
          </>
        )}
      </div>
    </aside>
  );
}

function InspectorSection({
  action,
  accessibleTitle,
  title,
  preferenceKey,
  meta,
  defaultOpen = false,
  children,
  sectionState,
}: {
  action?: ReactNode;
  accessibleTitle?: string;
  title: string;
  preferenceKey: string;
  meta?: ReactNode;
  defaultOpen?: boolean;
  children: ReactNode;
  sectionState: InspectorSectionState;
}) {
  const [fallbackOpen, setFallbackOpen] = useState(defaultOpen);
  const open =
    sectionState.kind === "controlled"
      ? sectionState.values[preferenceKey] ?? defaultOpen
      : fallbackOpen;

  function toggle() {
    const next = !open;
    if (sectionState.kind === "controlled") {
      sectionState.onChange(preferenceKey, next);
      return;
    }
    setFallbackOpen(next);
  }

  return (
    <section className="inspector-section">
      <div className="inspector-section-header">
        <button
          aria-label={accessibleTitle}
          type="button"
          className="inspector-section-trigger"
          aria-expanded={open}
          onClick={toggle}
        >
          <AppIcon icon={open ? ChevronDown : ChevronRight} size={12} />
          <span className="inspector-section-title">{title}</span>
          {meta !== undefined && (
            <span aria-hidden="true" className="inspector-section-meta">
              {meta}
            </span>
          )}
        </button>
        {action && <div className="inspector-section-action">{action}</div>}
      </div>
      {open && <div className="inspector-section-content">{children}</div>}
    </section>
  );
}
function PropertyRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="property-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function defaultSheetScope(sheet: ComposedSheet): SheetDesignScope {
  return sheet.activeSides === "both" ? "both" : sheet.activeSides;
}

function normalizeSheetScope(
  scope: SheetDesignScope,
  sheet: ComposedSheet,
): SheetDesignScope {
  if (sheet.activeSides === "both") return scope;
  return sheet.activeSides;
}

function formatSheetPageMetadata(sheet: SheetSnapshot | undefined) {
  if (!sheet || sheet.pageNumbers.length === 0) return null;

  const firstPage = sheet.pageNumbers[0];
  const lastPage = sheet.pageNumbers[sheet.pageNumbers.length - 1];
  if (sheet.role === "initial" && sheet.pageNumbers.length === 1) {
    return {
      accessibleLabel: `Lâmina inicial, Página ${firstPage}`,
      visualLabel: String(firstPage),
    };
  }
  if (sheet.role === "final" && sheet.pageNumbers.length === 1) {
    return {
      accessibleLabel: `Lâmina final, Página ${firstPage}`,
      visualLabel: String(firstPage),
    };
  }
  return firstPage === lastPage
    ? {
        accessibleLabel: `Página ${firstPage}`,
        visualLabel: String(firstPage),
      }
    : {
        accessibleLabel: `Páginas ${firstPage}–${lastPage}`,
        visualLabel: `${firstPage}–${lastPage}`,
      };
}
