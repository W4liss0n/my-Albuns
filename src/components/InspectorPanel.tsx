import { useState, type ReactNode } from "react";
import { Button } from "react-aria-components";

import type {
  ComposedPhoto,
  ComposedSheet,
  FrameSnapshot,
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
  sheetCount: number;
  photoCount: number;
  sheets: readonly ComposedSheet[];
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
  sheetCount,
  photoCount,
  sheets,
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
              <PropertyRow label="Lâminas" value={String(sheetCount)} />
              <PropertyRow
                label="Fotos posicionadas"
                value={String(photoCount)}
              />
              <PropertyRow label="Dimensão" value="60 × 30 cm" />
              <PropertyRow label="Resolução" value="300 DPI" />
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
