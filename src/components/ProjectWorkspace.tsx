import { useCallback, useEffect, useMemo, useState } from "react";
import { X } from "lucide-react";

import type {
  ExportPort,
  MediaPreview,
  MediaPreviewDemand,
  ProjectWindowPort,
} from "../application/projectPorts";
import type { ProjectDialogPort } from "../application/projectDialogPort";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { EditorProjection } from "../domain/project";
import {
  ActionButton,
  AppIcon,
  ApplicationHeader,
  InlineNotice,
} from "../ui";
import { AlbumCanvas } from "./AlbumCanvas";
import { ExportPreviewControl } from "./ExportPreviewControl";
import { InspectorPanel } from "./InspectorPanel";
import { micrometersToDisplayUnits } from "./measurementFormatting";
import { MediaPanel } from "./MediaPanel";
import { useProjectCloseController } from "./useProjectCloseController";
import { useProjectEditorController } from "./useProjectEditorController";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

type OpenApplicationMenu = "edit" | "file" | null;

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  projectDialogPort: ProjectDialogPort;
  exportPort: ExportPort;
  projectWindowPort: ProjectWindowPort;
  runProjectMutation: ProjectMutationRunner;
  mediaPreviews?: Readonly<Record<string, MediaPreview>>;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onProjectionChange(projection: EditorProjection): void;
  onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
}

export function ProjectWorkspace({
  projection,
  projectDialogPort,
  exportPort,
  projectWindowPort,
  runProjectMutation,
  mediaPreviews = {},
  onMediaDemandChange,
  onProjectionChange,
  onGraphicsUnavailable,
}: ProjectWorkspaceProps) {
  const [exportActive, setExportActive] = useState(false);
  const [openMenu, setOpenMenu] = useState<OpenApplicationMenu>(null);
  const [closeMessage, setCloseMessage] = useState<string | null>(null);
  const [canvasMediaDemand, setCanvasMediaDemand] =
    useState<MediaPreviewDemand>({
      visibleMediaIds: [],
      preloadMediaIds: [],
    });
  const [panelMediaDemand, setPanelMediaDemand] =
    useState<MediaPreviewDemand>({
      visibleMediaIds: [],
      preloadMediaIds: [],
    });
  const mediaPreviewUrls = useMemo(
    () =>
      Object.fromEntries(
        Object.entries(mediaPreviews).flatMap(([mediaId, preview]) =>
          preview.url ? [[mediaId, preview.url]] : [],
        ),
      ),
    [mediaPreviews],
  );
  useEffect(() => {
    if (!onMediaDemandChange) return;
    const visible = Array.from(
      new Set([
        ...canvasMediaDemand.visibleMediaIds,
        ...panelMediaDemand.visibleMediaIds,
      ]),
    );
    const visibleSet = new Set(visible);
    const preload = Array.from(
      new Set(
        [
          ...canvasMediaDemand.preloadMediaIds,
          ...panelMediaDemand.preloadMediaIds,
        ].filter((mediaId) => !visibleSet.has(mediaId)),
      ),
    );
    onMediaDemandChange({
      visibleMediaIds: visible,
      preloadMediaIds: preload,
    });
  }, [canvasMediaDemand, onMediaDemandChange, panelMediaDemand]);
  const reportCloseError = useCallback((value: string) => {
    setCloseMessage(value);
  }, []);
  const projectClose = useProjectCloseController({
    projectDialogPort,
    projectWindowPort,
    onProjectionChange,
    onError: reportCloseError,
  });
  useEffect(() => {
    if (projectClose.interactionBlocked) {
      setOpenMenu(null);
    }
  }, [projectClose.interactionBlocked]);
  const controller = useProjectEditorController({
    interactionBlocked: exportActive || projectClose.interactionBlocked,
    projection,
    runProjectMutation,
    onProjectionChange,
  });
  const workspacePanels = useWorkspacePanelLayout();
  const {
    busy,
    message,
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom,
    displayedPhotoPanX,
    photoCount,
  } = controller;
  const projectMetadata = projectAlbumMetadata(projection);

  return (
    <div className="app-shell">
      <ApplicationHeader
        context={projection.state.projectName}
        metadata={projectMetadata}
        status={projection.state.dirty ? "alterações não salvas" : "salvo"}
      />

      <div className="commandbar">
        <nav className="app-menu" aria-label="Menu principal">
          <div className="app-menu-entry">
            <button
              aria-expanded={openMenu === "file"}
              aria-haspopup="menu"
              disabled={
                Boolean(busy) ||
                exportActive ||
                projectClose.interactionBlocked
              }
              type="button"
              onClick={() =>
                setOpenMenu((current) =>
                  current === "file" ? null : "file",
                )
              }
            >
              Arquivo
            </button>
            {openMenu === "file" && (
              <div className="app-menu-popup" role="menu">
                <button
                  aria-label="Salvar"
                  disabled={
                    Boolean(busy) ||
                    exportActive ||
                    projectClose.interactionBlocked
                  }
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setOpenMenu(null);
                    void controller.save();
                  }}
                >
                  <span>Salvar</span>
                  <span className="app-menu-shortcut">Ctrl+S</span>
                </button>
                <span className="app-menu-separator" />
                <button
                  aria-label="Fechar Projeto"
                  disabled={projectClose.interactionBlocked}
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setOpenMenu(null);
                    void projectClose.requestClose();
                  }}
                >
                  <span>Fechar Projeto</span>
                  <span className="app-menu-shortcut">Ctrl+W</span>
                </button>
              </div>
            )}
          </div>
          <div className="app-menu-entry">
            <button
              aria-expanded={openMenu === "edit"}
              aria-haspopup="menu"
              disabled={
                Boolean(busy) ||
                exportActive ||
                projectClose.interactionBlocked
              }
              type="button"
              onClick={() =>
                setOpenMenu((current) =>
                  current === "edit" ? null : "edit",
                )
              }
            >
              Editar
            </button>
            {openMenu === "edit" && (
              <div className="app-menu-popup" role="menu">
                <button
                  aria-label="Desfazer"
                  disabled={
                    !projection.state.canUndo ||
                    Boolean(busy) ||
                    exportActive ||
                    projectClose.interactionBlocked
                  }
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setOpenMenu(null);
                    void controller.undo();
                  }}
                >
                  <span>Desfazer</span>
                  <span className="app-menu-shortcut">Ctrl+Z</span>
                </button>
                <button
                  aria-label="Refazer"
                  disabled={
                    !projection.state.canRedo ||
                    Boolean(busy) ||
                    exportActive ||
                    projectClose.interactionBlocked
                  }
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setOpenMenu(null);
                    void controller.redo();
                  }}
                >
                  <span>Refazer</span>
                  <span className="app-menu-shortcut">Ctrl+Shift+Z</span>
                </button>
              </div>
            )}
          </div>
          <button
            disabled={
              Boolean(busy) || exportActive || projectClose.interactionBlocked
            }
            type="button"
          >
            Inserir
          </button>
          <button
            disabled={
              Boolean(busy) || exportActive || projectClose.interactionBlocked
            }
            type="button"
          >
            Lâmina
          </button>
          <button
            disabled={
              Boolean(busy) || exportActive || projectClose.interactionBlocked
            }
            type="button"
          >
            Visualizar
          </button>
          <button
            disabled={
              Boolean(busy) || exportActive || projectClose.interactionBlocked
            }
            type="button"
          >
            Ajuda
          </button>
        </nav>
        <ExportPreviewControl
          dialogPort={projectDialogPort}
          disabled={Boolean(busy) || projectClose.interactionBlocked}
          exportPort={exportPort}
          onActiveChange={setExportActive}
          projectId={projection.state.projectId}
          sheetId={controller.canvasProps.centeredSheetId}
        />
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
            onMediaDemandChange={setCanvasMediaDemand}
            onGraphicsUnavailable={onGraphicsUnavailable}
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

        <InspectorPanel
          selectedFrame={selectedFrame}
          selectedComposedPhoto={selectedComposedPhoto}
          displayedPhotoZoom={displayedPhotoZoom}
          displayedPhotoPanX={displayedPhotoPanX}
          zoomCommitting={controller.zoomCommitting}
          photoCount={photoCount}
          document={projection.state.document}
          sheetStates={projection.state.album.sheets}
          sheets={projection.composition.sheets}
          frameBorder={projection.composition.frameBorder}
          focusedSheetId={controller.canvasProps.focusedSheetId}
          mediaPreviewUrls={mediaPreviewUrls}
          onBeginPhotoZoom={controller.beginZoomGesture}
          onUpdatePhotoZoom={controller.updateZoomGesture}
          onFinishPhotoZoom={controller.finishZoomGesture}
          onApplyDpi={controller.applyDpi}
          onNavigateToSheet={controller.navigateToSheet}
        />

        <MediaPanel
          mediaItems={projection.state.album.media}
          mediaUsage={projection.mediaUsage}
          mediaPreviews={mediaPreviews}
          onMediaDemandChange={setPanelMediaDemand}
          onFillPhoto={controller.fillMedia}
        />
      </div>

      {(busy || message || closeMessage) && (
        <InlineNotice
          className="operation-toast"
          floating
          role={message || closeMessage ? "alert" : "status"}
          tone={message || closeMessage ? "error" : "success"}
        >
          {busy && <span className="toast-spinner" aria-hidden="true" />}
          <div className="operation-toast__message">
            <strong>
              {message || closeMessage
                ? "A operação não foi concluída"
                : busy}
            </strong>
            <span>{closeMessage ?? message ?? "Aguarde…"}</span>
          </div>
          {!busy && (
            <ActionButton
              aria-label="Fechar mensagem"
              className="operation-toast__close"
              density="compact"
              variant="quiet"
              onClick={() => {
                setCloseMessage(null);
                controller.dismissFeedback();
              }}
            >
              <AppIcon icon={X} size={14} />
            </ActionButton>
          )}
        </InlineNotice>
      )}

    </div>
  );
}

const projectMetadataFormatter = new Intl.NumberFormat("pt-BR", {
  maximumFractionDigits: 2,
  useGrouping: false,
});

function projectAlbumMetadata(projection: EditorProjection) {
  const { album, document } = projection.state;
  const width = projectMetadataFormatter.format(
    micrometersToDisplayUnits(
      document.sheetWidthUm / 2,
      document.displayUnit,
    ),
  );
  const height = projectMetadataFormatter.format(
    micrometersToDisplayUnits(
      document.sheetHeightUm,
      document.displayUnit,
    ),
  );
  const sheetLabel = album.sheets.length === 1 ? "Lâmina" : "Lâminas";
  return `${width}×${height} ${document.displayUnit} · ${album.sheets.length} ${sheetLabel}`;
}
