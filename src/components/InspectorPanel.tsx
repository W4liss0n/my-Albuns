import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
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
import {
  readInspectorSectionPreference,
  writeInspectorSectionPreference,
} from "../state/workspacePreferences";
import { ActionButton, AppIcon, EmptyState } from "../ui";
import { AlbumDesignForm } from "./AlbumDesignForm";
import { AlbumInformationForm } from "./AlbumInformationForm";
import { SheetPreview } from "./SheetPreview";
import {
  SheetDesignInspector,
  type SheetDesignScope,
} from "./SheetDesignInspector";
import { inactiveSideCssGradient } from "./sheetVisualStyle";
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
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  onBeginPhotoZoom(): void;
  onUpdatePhotoZoom(value: number): void;
  onFinishPhotoZoom(): void | Promise<void>;
  onApplyAlbumInformation(
    information: AlbumInformation,
    baseline: AlbumInformation,
    impact: AlbumInformationImpact,
  ): void | Promise<unknown>;
  onApplyAlbumDesign(
    visualDefaults: ProjectedVisualDefaults,
  ): void | Promise<unknown>;
  onValidateAlbumInformation(
    information: AlbumInformation,
  ): Promise<AlbumInformationValidation>;
  onPresentationUnitChange(unit: DisplayUnit | null): void;
  onNavigateToSheet(sheetId: string): void;
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
  mediaPreviewUrls = {},
  onBeginPhotoZoom,
  onUpdatePhotoZoom,
  onFinishPhotoZoom,
  onApplyAlbumInformation,
  onApplyAlbumDesign,
  onPresentationUnitChange,
  onValidateAlbumInformation,
  onNavigateToSheet,
}: InspectorPanelProps) {
  const [informationDirty, setInformationDirty] = useState(false);
  const [designDirty, setDesignDirty] = useState(false);
  const [sheetScopeSelection, setSheetScopeSelection] = useState<{
    sheetId: string;
    scope: SheetDesignScope;
  } | null>(null);
  const sheetStateById = new Map(
    sheetStates.map((sheet) => [sheet.id, sheet] as const),
  );
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
              defaultOpen
            >
              <div className="inspector-subsections">
                <AlbumInformationForm
                  document={document}
                  formId={ALBUM_INFORMATION_FORM_ID}
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
              defaultOpen
            >
              <AlbumDesignForm
                document={document}
                presentationUnit={presentationUnit}
                formId={ALBUM_DESIGN_FORM_ID}
                mediaItems={mediaItems}
                mediaPreviewUrls={mediaPreviewUrls}
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
                  data-placeholder-feature="reorder-sheets-from-grid"
                >
                  {sheets.map((sheet) => {
                    const number = String(sheet.number).padStart(2, "0");
                    const pageMetadata = formatSheetPageMetadata(
                      sheetStateById.get(sheet.sheetId),
                    );
                    const visualPageLabel =
                      pageMetadata?.visualLabel ?? `Lâmina ${number}`;
                    const accessiblePageLabel =
                      pageMetadata?.accessibleLabel ?? `Lâmina ${number}`;
                    const active = sheet.sheetId === focusedSheetId;
                    return (
                      <Button
                        aria-current={active ? "true" : undefined}
                        aria-label={`Ir para Lâmina ${number}, ${accessiblePageLabel}`}
                        key={sheet.sheetId}
                        className={active ? "sheet-tile active" : "sheet-tile"}
                        data-active-sides={sheet.activeSides}
                        style={
                          sheet.activeSides === "both"
                            ? undefined
                            : ({
                                "--sheet-inactive-side-gradient":
                                  inactiveSideCssGradient(sheet.activeSides),
                              } as CSSProperties)
                        }
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
                    );
                  })}
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
}: {
  action?: ReactNode;
  accessibleTitle?: string;
  title: string;
  preferenceKey: string;
  meta?: ReactNode;
  defaultOpen?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(() =>
    readInspectorSectionPreference(preferenceKey, defaultOpen),
  );

  function toggle() {
    setOpen((current) => {
      const next = !current;
      writeInspectorSectionPreference(preferenceKey, next);
      return next;
    });
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
