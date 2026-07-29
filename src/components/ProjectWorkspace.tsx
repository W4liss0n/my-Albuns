import { useState } from "react";
import { Button } from "react-aria-components";

import type { EditorProjection, ProjectBridge } from "../domain/project";
import { AlbumCanvas } from "./AlbumCanvas";
import { SheetPreview } from "./SheetPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  bridge: ProjectBridge;
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  onProjectionChange(projection: EditorProjection): void;
}

export function ProjectWorkspace({
  projection,
  bridge,
  mediaPreviewUrls = {},
  onProjectionChange,
}: ProjectWorkspaceProps) {
  const controller = useProjectEditorController({
    projection,
    bridge,
    onProjectionChange,
  });
  const workspacePanels = useWorkspacePanelLayout();
  const {
    busy,
    message,
    exportResult,
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom,
    displayedPhotoPanX,
    sheetCount,
    photoCount,
  } = controller;
  const focusedSheetId = controller.canvasProps.focusedSheetId;

  return (
    <div className="app-shell">
      <header className="titlebar">
        <nav className="app-menu" aria-label="Menu principal">
          <button type="button">Arquivo</button>
          <button type="button">Editar</button>
          <button type="button">Lâmina</button>
          <button type="button">Exibir</button>
          <button type="button">Ferramentas</button>
          <button type="button">Ajuda</button>
        </nav>
      </header>

      <div className="commandbar">
        <div className="command-group">
          <Button
            className="icon-command"
            aria-label="Desfazer"
            isDisabled={!projection.state.canUndo || Boolean(busy)}
            onPress={controller.undo}
          >
            ↶
          </Button>
          <Button
            className="icon-command"
            aria-label="Refazer"
            isDisabled={!projection.state.canRedo || Boolean(busy)}
            onPress={controller.redo}
          >
            ↷
          </Button>
        </div>
        <span className="command-divider" />
        <div className="tool-active">
          <span aria-hidden="true">↖</span>
          <span>Selecionar</span>
        </div>
        <div className="command-spacer" />
        <Button
          className="primary-command"
          onPress={controller.exportPreview}
          isDisabled={Boolean(busy)}
        >
          <span aria-hidden="true">⇧</span>
          Exportar prova
        </Button>
      </div>

      <div
        className="workspace-grid"
        ref={workspacePanels.workspaceRef}
        style={workspacePanels.style}
      >
        <section
          id="continuous-canvas"
          className="canvas-section"
          aria-label="Área de composição"
        >
          <AlbumCanvas
            {...controller.canvasProps}
            mediaPreviewUrls={mediaPreviewUrls}
          />
        </section>

        <WorkspacePanelSplitter
          panel="media"
          size={workspacePanels.sizes.media}
          onResizeStart={workspacePanels.beginResize}
          onResizeBy={workspacePanels.resizeBy}
        />

        <WorkspacePanelSplitter
          panel="inspector"
          size={workspacePanels.sizes.inspector}
          onResizeStart={workspacePanels.beginResize}
          onResizeBy={workspacePanels.resizeBy}
        />

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
                  <h2>{selectedFrame.photo?.name ?? "Frame placeholder"}</h2>
                </div>
                <InspectorSection
                  key="frame-photo-design"
                  title="Design"
                  preferenceKey="frame-photo.design"
                  defaultOpen
                >
                  <PropertyRow
                    label="Frame"
                    value={selectedFrame.id
                      .replace("frame-", "")
                      .toUpperCase()}
                  />
                  <PropertyRow
                    label="Pan horizontal"
                    value={`${Math.round(displayedPhotoPanX * 100)}%`}
                  />
                  {selectedFrame.photo && selectedComposedPhoto && (
                    <label className="photo-zoom-control">
                      <span className="photo-zoom-label">
                        <span>Zoom da Foto</span>
                        <output>
                          {Math.round(displayedPhotoZoom * 100)}%
                        </output>
                      </span>
                      <input
                        type="range"
                        aria-label="Zoom da Foto"
                        min={
                          selectedComposedPhoto.placement.zoomRange.minimum *
                          100
                        }
                        max={
                          selectedComposedPhoto.placement.zoomRange.maximum *
                          100
                        }
                        step="1"
                        value={Math.round(displayedPhotoZoom * 100)}
                        disabled={controller.zoomCommitting}
                        onPointerDown={controller.beginZoomGesture}
                        onChange={(event) =>
                          controller.updateZoomGesture(
                            Number(event.currentTarget.value) / 100,
                          )
                        }
                        onPointerUp={controller.finishZoomGesture}
                        onKeyDown={(event) => {
                          if (
                            [
                              "ArrowLeft",
                              "ArrowRight",
                              "ArrowUp",
                              "ArrowDown",
                              "Home",
                              "End",
                              "PageUp",
                              "PageDown",
                            ].includes(event.key)
                          ) {
                            controller.beginZoomGesture();
                          }
                        }}
                        onKeyUp={(event) => {
                          if (
                            [
                              "ArrowLeft",
                              "ArrowRight",
                              "ArrowUp",
                              "ArrowDown",
                              "Home",
                              "End",
                              "PageUp",
                              "PageDown",
                            ].includes(event.key)
                          ) {
                            void controller.finishZoomGesture();
                          }
                        }}
                        onBlur={controller.finishZoomGesture}
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
                    {projection.composition.sheets.map((sheet) => {
                      return (
                        <Button
                          key={sheet.sheetId}
                          className={
                            sheet.sheetId === focusedSheetId
                              ? "sheet-tile active"
                              : "sheet-tile"
                          }
                          onPress={() =>
                            controller.navigateToSheet(sheet.sheetId)
                          }
                        >
                          <SheetPreview
                            sheet={sheet}
                            mediaPreviewUrls={mediaPreviewUrls}
                          />
                          <span>{String(sheet.number).padStart(2, "0")}</span>
                        </Button>
                      );
                    })}
                  </div>
                </InspectorSection>
              </>
            )}
          </div>
        </aside>

        <section
          id="media-panel"
          className="media-panel"
          aria-label="Painel de imagens"
        >
          <div className="media-panel-head">
            <div className="media-tabs">
              <button className="active" type="button">
                Fotos
              </button>
              <button type="button">Decorativos</button>
            </div>
            <label className="media-search">
              <span aria-hidden="true">⌕</span>
              <input aria-label="Buscar imagens" placeholder="Buscar imagens" />
            </label>
          </div>
          <div className="media-strip">
            {projection.state.album.media.map((media) => (
              <button
                className="media-card"
                type="button"
                key={media.id}
                onDoubleClick={() => controller.fillMedia(media.id)}
                title="Duplo clique para preencher o placeholder mais à esquerda da Lâmina centralizada"
              >
                <span
                  className="media-thumb"
                  style={{
                    background: `linear-gradient(135deg, ${media.palette[0]}, ${media.palette[1]} 56%, ${media.palette[2]})`,
                  }}
                >
                  {mediaPreviewUrls[media.id] && (
                    <img
                      alt=""
                      draggable="false"
                      loading="lazy"
                      src={mediaPreviewUrls[media.id]}
                    />
                  )}
                </span>
                <span className="media-meta">
                  <strong>{media.name}</strong>
                  <small>{media.usageCount} usos</small>
                </span>
              </button>
            ))}
            <div className="media-tip">
              <kbd>2×</kbd>
              <span>
                Preenche o placeholder mais à esquerda da Lâmina centralizada
              </span>
            </div>
          </div>
        </section>
      </div>

      {(busy || message || exportResult) && (
        <div
          className={`operation-toast ${message ? "error" : ""}`}
          role={message ? "alert" : "status"}
        >
          {busy && <span className="toast-spinner" aria-hidden="true" />}
          <div>
            <strong>
              {message
                ? "A operação não foi concluída"
                : busy
                  ? busy
                  : "Exportação concluída"}
            </strong>
            <span>
              {message ??
                (exportResult
                  ? `${exportResult.widthPx} × ${exportResult.heightPx}px · ${exportResult.outputPath}`
                  : "Aguarde…")}
            </span>
          </div>
          {!busy && (
            <button
              type="button"
              aria-label="Fechar mensagem"
              onClick={controller.dismissFeedback}
            >
              ×
            </button>
          )}
        </div>
      )}
    </div>
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
  children: React.ReactNode;
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

function readInspectorSectionPreference(
  preferenceKey: string,
  fallback: boolean,
) {
  try {
    const stored = window.localStorage.getItem(
      `myalbuns.inspector.${preferenceKey}`,
    );
    return stored === null ? fallback : stored === "open";
  } catch {
    return fallback;
  }
}

function writeInspectorSectionPreference(
  preferenceKey: string,
  open: boolean,
) {
  try {
    window.localStorage.setItem(
      `myalbuns.inspector.${preferenceKey}`,
      open ? "open" : "closed",
    );
  } catch {
    // The in-memory preference remains usable when storage is unavailable.
  }
}

function PropertyRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="property-row">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}
