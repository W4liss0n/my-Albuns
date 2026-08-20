import { useState, type CSSProperties, type ReactNode } from "react";
import { Button } from "react-aria-components";
import { ChevronDown, ChevronRight, PanelsTopLeft } from "lucide-react";

import type {
  AlbumInformation,
  AlbumInformationImpact,
  AlbumInformationValidation,
  ComposedPhoto,
  ComposedSheet,
  DocumentSnapshot,
  FrameSnapshot,
  ProjectedFrameBorder,
  ProjectedVisualDefaults,
  SheetSnapshot,
} from "../domain/project";
import {
  readInspectorSectionPreference,
  writeInspectorSectionPreference,
} from "../state/workspacePreferences";
import { ActionButton, AppIcon, EmptyState } from "../ui";
import { AlbumInformationForm } from "./AlbumInformationForm";
import { micrometersToDisplayUnits } from "./measurementFormatting";
import { SheetPreview } from "./SheetPreview";
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

export interface InspectorPanelProps {
  selectedFrame: FrameSnapshot | null;
  selectedComposedPhoto: ComposedPhoto | null;
  displayedPhotoZoom: number;
  displayedPhotoPanX: number;
  zoomCommitting: boolean;
  document: DocumentSnapshot;
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
  onValidateAlbumInformation(
    information: AlbumInformation,
  ): Promise<AlbumInformationValidation>;
  onNavigateToSheet(sheetId: string): void;
}

export function InspectorPanel({
  selectedFrame,
  selectedComposedPhoto,
  displayedPhotoZoom,
  displayedPhotoPanX,
  zoomCommitting,
  document,
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
  onValidateAlbumInformation,
  onNavigateToSheet,
}: InspectorPanelProps) {
  const [informationDirty, setInformationDirty] = useState(false);
  const sheetStateById = new Map(
    sheetStates.map((sheet) => [sheet.id, sheet] as const),
  );

  return (
    <aside
      id="contextual-panel"
      className="inspector"
      aria-label="Painel contextual"
    >
      <div className="inspector-scroll">
        {selectedFrame ? (
          <>
            <div className="context-heading">
              <span>Frame selecionado</span>
              <h2>{selectedComposedPhoto?.name ?? "Frame placeholder"}</h2>
            </div>
            <InspectorSection
              key="frame-photo-design"
              title="Design"
              preferenceKey="frame-photo.design"
              defaultOpen
            >
              <PropertyRow
                label="Frame"
                value={selectedFrame.id.replace("frame-", "").toUpperCase()}
              />
              <PropertyRow
                label="Pan horizontal"
                value={`${Math.round(displayedPhotoPanX * 100)}%`}
              />
              {selectedFrame.photo && selectedComposedPhoto && (
                <label className="photo-zoom-control">
                  <span className="photo-zoom-label">
                    <span>Zoom da Foto</span>
                    <output>{Math.round(displayedPhotoZoom * 100)}%</output>
                  </span>
                  <input
                    type="range"
                    aria-label="Zoom da Foto"
                    min={
                      selectedComposedPhoto.placement.zoomRange.minimum * 100
                    }
                    max={
                      selectedComposedPhoto.placement.zoomRange.maximum * 100
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
        ) : (
          <>
            <InspectorSection
              action={
                <ActionButton
                  density="compact"
                  disabled={!informationDirty}
                  form={ALBUM_INFORMATION_FORM_ID}
                  type="submit"
                  variant="quiet"
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
                  onReadyChange={setInformationDirty}
                  onValidate={onValidateAlbumInformation}
                  sheetStates={sheetStates}
                />
              </div>
            </InspectorSection>
            <InspectorSection
              action={
                <ActionButton
                  data-placeholder-feature="album-design-apply"
                  density="compact"
                  disabled
                  variant="quiet"
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
              <AlbumVisualDefaultsPlaceholder
                displayUnit={document.displayUnit}
                visualDefaults={visualDefaults}
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
function InspectorReadout({
  fieldPlaceholder = false,
  label,
  value,
}: {
  fieldPlaceholder?: boolean;
  label: string;
  value: string;
}) {
  return (
    <div className="inspector-readout-field">
      <span>{label}</span>
      <output
        aria-label={label}
        className={[
          "inspector-readout",
          fieldPlaceholder ? "ui-field-control" : "",
          fieldPlaceholder
            ? "inspector-readout--field-placeholder"
            : "inspector-readout--integrated",
        ]
          .filter(Boolean)
          .join(" ")}
        title={value}
      >
        {value}
      </output>
    </div>
  );
}

function AlbumVisualDefaultsPlaceholder({
  displayUnit,
  visualDefaults,
}: {
  displayUnit: DocumentSnapshot["displayUnit"];
  visualDefaults: ProjectedVisualDefaults;
}) {
  const visualState = albumVisualDefaultsView(visualDefaults);

  return (
    <div
      className="inspector-subsections album-visual-defaults-placeholder"
      data-placeholder-feature="album-design-visual-defaults"
    >
      <section className="inspector-subsection">
        <h3>Padrões visuais</h3>
        <VisualDefaultPlaceholder
          currentLabel={visualState.background.label}
          label="Background"
          previewClassName="visual-default-picker__preview--background"
          previewStyle={visualState.background.style}
          recentLabel="backgrounds recentes"
        />
        <VisualDefaultPlaceholder
          currentLabel={visualState.overlay.label}
          label="Overlay"
          previewClassName={visualState.overlay.previewClassName}
          recentLabel="overlays recentes"
        />
      </section>
      <section className="inspector-subsection">
        <h3>Padrão dos Frames</h3>
        <span className="album-design-label">Borda padrão</span>
        <div className="frame-border-placeholder" aria-hidden="true">
          <span
            className={
              visualState.frameBorder.enabled
                ? "frame-border-placeholder__option"
                : "frame-border-placeholder__option is-current"
            }
          >
            Sem borda
          </span>
          <span
            className={
              visualState.frameBorder.enabled
                ? "frame-border-placeholder__option is-current"
                : "frame-border-placeholder__option"
            }
          >
            Com borda
          </span>
        </div>
        {visualState.frameBorder.enabled && (
          <div className="frame-border-summary">
            <span
              aria-hidden="true"
              className="frame-border-summary__swatch"
              style={{ background: visualState.frameBorder.rgb }}
            />
            <InspectorReadout
              fieldPlaceholder
              label="Espessura"
              value={formatMeasurement(
                visualState.frameBorder.widthUm,
                displayUnit,
              )}
            />
          </div>
        )}
      </section>
    </div>
  );
}

function VisualDefaultPlaceholder({
  currentLabel,
  label,
  previewClassName,
  previewStyle,
  recentLabel,
}: {
  currentLabel: string;
  label: string;
  previewClassName: string;
  previewStyle?: CSSProperties;
  recentLabel: string;
}) {
  return (
    <div className="visual-default-field">
      <span className="album-design-label">{label}</span>
      <div className="visual-default-picker" aria-hidden="true">
        <div className="visual-default-picker__current">
          <span
            className={`visual-default-picker__preview ${previewClassName}`}
            style={previewStyle}
          />
          <span>{currentLabel}</span>
        </div>
        <span className="visual-default-picker__divider" />
        <div className="visual-default-picker__recent">
          <div className="visual-default-picker__tiles">
            <span className="visual-default-picker__tile visual-default-picker__tile--light-hatch" />
            <span className="visual-default-picker__tile visual-default-picker__tile--dark-hatch" />
            <span className="visual-default-picker__add">+</span>
          </div>
          <span>{recentLabel}</span>
        </div>
      </div>
    </div>
  );
}

function albumVisualDefaultsView(defaults: ProjectedVisualDefaults) {
  const frameBorder =
    defaults.frameBorder.kind === "solid"
      ? {
          enabled: true as const,
          rgb: defaults.frameBorder.rgb,
          widthUm: defaults.frameBorder.widthUm,
        }
      : { enabled: false as const };

  return {
    background: backgroundVisualState(defaults.background),
    frameBorder,
    overlay: overlayVisualState(defaults.overlay),
  };
}

function backgroundVisualState(
  background: ProjectedVisualDefaults["background"],
): { label: string; style?: CSSProperties } {
  if (background.scope === "bothSides") {
    return background.both.kind === "color"
      ? { label: "cor", style: { background: background.both.rgb } }
      : { label: "imagem" };
  }

  if (background.left.kind !== "color" || background.right.kind !== "color") {
    return { label: "por lado" };
  }

  return {
    label: "por lado",
    style: {
      background: `linear-gradient(90deg, ${background.left.rgb} 0 50%, ${background.right.rgb} 50% 100%)`,
    },
  };
}

function overlayVisualState(
  overlay: ProjectedVisualDefaults["overlay"],
): { label: string; previewClassName: string } {
  if (overlay.scope === "bothSides") {
    return overlay.both === null
      ? {
          label: "sem",
          previewClassName: "visual-default-picker__preview--none",
        }
      : {
          label: "imagem",
          previewClassName: "visual-default-picker__preview--overlay",
        };
  }
  if (overlay.left === null && overlay.right === null) {
    return {
      label: "sem",
      previewClassName: "visual-default-picker__preview--none",
    };
  }
  return {
    label: "por lado",
    previewClassName: "visual-default-picker__preview--overlay",
  };
}

function PropertyRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="property-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

const measurementFormatter = new Intl.NumberFormat("pt-BR", {
  maximumFractionDigits: 6,
  useGrouping: false,
});

function formatMeasurement(
  micrometers: number,
  unit: DocumentSnapshot["displayUnit"],
) {
  return `${measurementFormatter.format(
    micrometersToDisplayUnits(micrometers, unit),
  )} ${unit}`;
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
