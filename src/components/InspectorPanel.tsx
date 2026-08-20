import { useState, type CSSProperties, type ReactNode } from "react";
import { Button } from "react-aria-components";
import { ChevronDown, ChevronRight, PanelsTopLeft } from "lucide-react";

import type {
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
import { AppIcon, EmptyState } from "../ui";
import { DocumentDpiControl } from "./DocumentDpiControl";
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

export interface InspectorPanelProps {
  projectName: string;
  selectedFrame: FrameSnapshot | null;
  selectedComposedPhoto: ComposedPhoto | null;
  displayedPhotoZoom: number;
  displayedPhotoPanX: number;
  zoomCommitting: boolean;
  photoCount: number;
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
  onApplyDpi(dpi: number): void | Promise<void>;
  onNavigateToSheet(sheetId: string): void;
}

export function InspectorPanel({
  projectName,
  selectedFrame,
  selectedComposedPhoto,
  displayedPhotoZoom,
  displayedPhotoPanX,
  zoomCommitting,
  photoCount,
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
  onApplyDpi,
  onNavigateToSheet,
}: InspectorPanelProps) {
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
              accessibleTitle="Informações do Álbum"
              key="album-information"
              title="Informações do Álbum"
              preferenceKey="album.information"
              defaultOpen
            >
              <AlbumInformation
                document={document}
                photoCount={photoCount}
                projectName={projectName}
                sheetStates={sheetStates}
              />
            </InspectorSection>
            <InspectorSection
              accessibleTitle="Design do Álbum"
              key="album-design"
              title="Design do Álbum"
              preferenceKey="album.design"
              defaultOpen
            >
              <div className="album-design-content">
                <div className="document-settings-group">
                  <h3>Documento</h3>
                  <DocumentDpiControl
                    dpi={document.dpi}
                    onApplyDpi={onApplyDpi}
                  />
                </div>
                <AlbumVisualDefaultsPlaceholder
                  displayUnit={document.displayUnit}
                  visualDefaults={visualDefaults}
                />
              </div>
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
  accessibleTitle,
  title,
  preferenceKey,
  meta,
  defaultOpen = false,
  children,
}: {
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
      {open && <div className="inspector-section-content">{children}</div>}
    </section>
  );
}

function AlbumInformation({
  document,
  photoCount,
  projectName,
  sheetStates,
}: {
  document: DocumentSnapshot;
  photoCount: number;
  projectName: string;
  sheetStates: readonly SheetSnapshot[];
}) {
  const firstSheet = sheetStates[0];
  const lastSheet = sheetStates[sheetStates.length - 1];
  const frames = sheetStates.flatMap((sheet) => sheet.frames);
  const placeholderCount = frames.filter((frame) => frame.photo === null).length;
  const positionedPhotoCount = frames.length - placeholderCount;

  return (
    <div className="inspector-readout-grid">
      <InspectorReadout label="Nome do Projeto" value={projectName} wide />
      <InspectorReadout label="Lâminas" value={String(sheetStates.length)} />
      <InspectorReadout label="Páginas" value={String(pageCount(sheetStates))} />
      <InspectorReadout
        label="Fotos importadas"
        value={String(photoCount)}
      />
      <InspectorReadout
        label="Fotos posicionadas"
        value={String(positionedPhotoCount)}
      />
      <InspectorReadout
        label="Dimensão da Lâmina"
        value={formatDimensions(
          document.sheetWidthUm,
          document.sheetHeightUm,
          document.displayUnit,
        )}
        wide
      />
      <InspectorReadout
        label="Dimensão da Página"
        value={formatDimensions(
          document.sheetWidthUm / 2,
          document.sheetHeightUm,
          document.displayUnit,
        )}
        wide
      />
      <InspectorReadout label="Unidade" value={document.displayUnit} />
      <InspectorReadout label="Resolução" value={`${document.dpi} DPI`} />
      <InspectorReadout
        label="Primeira Lâmina"
        value={sheetFormat(firstSheet)}
      />
      <InspectorReadout
        label="Última Lâmina"
        value={sheetFormat(lastSheet)}
      />
      <InspectorReadout
        label="Sangria"
        value={formatMeasurement(document.bleedUm, document.displayUnit)}
      />
      <InspectorReadout
        label="Área de segurança"
        value={formatMeasurement(document.safetyUm, document.displayUnit)}
      />
      <InspectorReadout
        label="Frames placeholder"
        value={String(placeholderCount)}
        wide
      />
    </div>
  );
}

function InspectorReadout({
  label,
  value,
  wide = false,
}: {
  label: string;
  value: string;
  wide?: boolean;
}) {
  return (
    <div
      className={
        wide
          ? "inspector-readout-field inspector-readout-field--wide"
          : "inspector-readout-field"
      }
    >
      <span>{label}</span>
      <output
        aria-label={label}
        className="ui-field-control inspector-readout"
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
  const hasOverlay =
    visualDefaults.overlay.scope === "bothSides"
      ? visualDefaults.overlay.both !== null
      : visualDefaults.overlay.left !== null ||
        visualDefaults.overlay.right !== null;
  const frameBorderEnabled = visualDefaults.frameBorder.kind === "solid";

  return (
    <div
      className="album-visual-defaults-placeholder"
      data-placeholder-feature="album-design-visual-defaults"
    >
      <div className="album-design-group">
        <h3>Padrões visuais</h3>
        <VisualDefaultPlaceholder
          currentLabel="atual"
          label="Background"
          previewClassName="visual-default-picker__preview--background"
          previewStyle={backgroundPreviewStyle(visualDefaults.background)}
          recentLabel="backgrounds recentes"
        />
        <VisualDefaultPlaceholder
          currentLabel={hasOverlay ? "atual" : "sem"}
          label="Overlay"
          previewClassName={
            hasOverlay
              ? "visual-default-picker__preview--overlay"
              : "visual-default-picker__preview--none"
          }
          recentLabel="overlays recentes"
        />
      </div>
      <div className="album-design-group">
        <h3>Padrão dos Frames</h3>
        <span className="album-design-label">Borda padrão</span>
        <div className="frame-border-placeholder" aria-hidden="true">
          <span
            className={
              frameBorderEnabled
                ? "frame-border-placeholder__option"
                : "frame-border-placeholder__option is-current"
            }
          >
            Sem borda
          </span>
          <span
            className={
              frameBorderEnabled
                ? "frame-border-placeholder__option is-current"
                : "frame-border-placeholder__option"
            }
          >
            Com borda
          </span>
        </div>
        {visualDefaults.frameBorder.kind === "solid" && (
          <div className="frame-border-summary">
            <span
              aria-hidden="true"
              className="frame-border-summary__swatch"
              style={{ background: visualDefaults.frameBorder.rgb }}
            />
            <InspectorReadout
              label="Espessura"
              value={formatMeasurement(
                visualDefaults.frameBorder.widthUm,
                displayUnit,
              )}
            />
          </div>
        )}
      </div>
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
            <span className="visual-default-picker__tile visual-default-picker__tile--one" />
            <span className="visual-default-picker__tile visual-default-picker__tile--two" />
            <span className="visual-default-picker__add">+</span>
          </div>
          <span>{recentLabel}</span>
        </div>
      </div>
    </div>
  );
}

function backgroundPreviewStyle(
  background: ProjectedVisualDefaults["background"],
): CSSProperties | undefined {
  if (background.scope === "bothSides") {
    return background.both.kind === "color"
      ? { background: background.both.rgb }
      : undefined;
  }

  if (background.left.kind !== "color" || background.right.kind !== "color") {
    return undefined;
  }

  return {
    background: `linear-gradient(90deg, ${background.left.rgb} 0 50%, ${background.right.rgb} 50% 100%)`,
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

function formatDimensions(
  widthUm: number,
  heightUm: number,
  unit: DocumentSnapshot["displayUnit"],
) {
  const width = measurementFormatter.format(
    micrometersToDisplayUnits(widthUm, unit),
  );
  const height = measurementFormatter.format(
    micrometersToDisplayUnits(heightUm, unit),
  );
  return `${width} × ${height} ${unit}`;
}

function pageCount(sheets: readonly SheetSnapshot[]) {
  return sheets.reduce((total, sheet) => total + sheet.pageNumbers.length, 0);
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

function sheetFormat(sheet: SheetSnapshot | undefined) {
  if (!sheet) return "—";
  return sheet.activeSides === "both" ? "Lâmina dupla" : "Página única";
}
