import { useState, type ReactNode } from "react";
import { Button } from "react-aria-components";

import type {
  ComposedPhoto,
  ComposedSheet,
  DocumentSnapshot,
  FrameSnapshot,
  ProjectedFrameBorder,
  SheetSnapshot,
} from "../domain/project";
import {
  readInspectorSectionPreference,
  writeInspectorSectionPreference,
} from "../state/workspacePreferences";
import { SheetPreview } from "./SheetPreview";
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
  focusedSheetId: string | null;
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  onBeginPhotoZoom(): void;
  onUpdatePhotoZoom(value: number): void;
  onFinishPhotoZoom(): void | Promise<void>;
  onNavigateToSheet(sheetId: string): void;
}

export function InspectorPanel({
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
  focusedSheetId,
  mediaPreviewUrls = {},
  onBeginPhotoZoom,
  onUpdatePhotoZoom,
  onFinishPhotoZoom,
  onNavigateToSheet,
}: InspectorPanelProps) {
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
              key="album-information"
              title="Informações do Álbum"
              preferenceKey="album.information"
              defaultOpen
            >
              <PropertyRow label="Lâminas" value={String(sheetStates.length)} />
              <PropertyRow
                label="Páginas"
                value={String(pageCount(sheetStates))}
              />
              <PropertyRow
                label="Fotos posicionadas"
                value={String(photoCount)}
              />
              <PropertyRow
                label="Dimensão da Lâmina"
                value={formatDimensions(
                  document.sheetWidthUm,
                  document.sheetHeightUm,
                  document.displayUnit,
                )}
              />
              <PropertyRow
                label="Dimensão da Página"
                value={formatDimensions(
                  document.sheetWidthUm / 2,
                  document.sheetHeightUm,
                  document.displayUnit,
                )}
              />
              <PropertyRow label="Unidade" value={document.displayUnit} />
              <PropertyRow
                label="Resolução"
                value={`${document.dpi} DPI`}
              />
              <PropertyRow
                label="Primeira Lâmina"
                value={sheetFormat(sheetStates[0])}
              />
              <PropertyRow
                label="Última Lâmina"
                value={sheetFormat(sheetStates[sheetStates.length - 1])}
              />
              <PropertyRow
                label="Sangria"
                value={formatMeasurement(
                  document.bleedUm,
                  document.displayUnit,
                )}
              />
              <PropertyRow
                label="Área de segurança"
                value={formatMeasurement(
                  document.safetyUm,
                  document.displayUnit,
                )}
              />
            </InspectorSection>
            <InspectorSection
              key="album-sheet-grid"
              title="Grade de Lâminas"
              preferenceKey="album.sheet-grid"
              defaultOpen
            >
              <div className="sheet-grid">
                {sheets.map((sheet) => (
                  <Button
                    key={sheet.sheetId}
                    className={
                      sheet.sheetId === focusedSheetId
                        ? "sheet-tile active"
                        : "sheet-tile"
                    }
                    onPress={() => onNavigateToSheet(sheet.sheetId)}
                  >
                    <SheetPreview
                      frameBorder={frameBorder}
                      sheet={sheet}
                      mediaPreviewUrls={mediaPreviewUrls}
                    />
                    <span>{String(sheet.number).padStart(2, "0")}</span>
                  </Button>
                ))}
              </div>
            </InspectorSection>
          </>
        )}
      </div>
    </aside>
  );
}

function InspectorSection({
  title,
  preferenceKey,
  defaultOpen = false,
  children,
}: {
  title: string;
  preferenceKey: string;
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
        type="button"
        className="inspector-section-trigger"
        aria-expanded={open}
        onClick={toggle}
      >
        <span>{title}</span>
        <span aria-hidden="true">{open ? "−" : "+"}</span>
      </button>
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

const MICROMETERS_PER_UNIT = {
  mm: 1_000,
  cm: 10_000,
  in: 25_400,
} as const;

const measurementFormatter = new Intl.NumberFormat("pt-BR", {
  maximumFractionDigits: 6,
  useGrouping: false,
});

function formatMeasurement(
  micrometers: number,
  unit: DocumentSnapshot["displayUnit"],
) {
  return `${measurementFormatter.format(
    micrometers / MICROMETERS_PER_UNIT[unit],
  )} ${unit}`;
}

function formatDimensions(
  widthUm: number,
  heightUm: number,
  unit: DocumentSnapshot["displayUnit"],
) {
  const width = measurementFormatter.format(
    widthUm / MICROMETERS_PER_UNIT[unit],
  );
  const height = measurementFormatter.format(
    heightUm / MICROMETERS_PER_UNIT[unit],
  );
  return `${width} × ${height} ${unit}`;
}

function pageCount(sheets: readonly SheetSnapshot[]) {
  return sheets.reduce(
    (total, sheet) => total + (sheet.activeSides === "both" ? 2 : 1),
    0,
  );
}

function sheetFormat(sheet: SheetSnapshot | undefined) {
  if (!sheet) return "—";
  return sheet.activeSides === "both" ? "Lâmina dupla" : "Página única";
}
