import { useCallback, useEffect, useMemo, useState } from "react";
import { Button } from "react-aria-components";

import type {
  ExportPipelinePort,
  MediaPreview,
  MediaPreviewDemand,
  ProjectCorePort,
  ProjectWindowPort,
} from "../application/projectPorts";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { EditorProjection } from "../domain/project";
import { AlbumCanvas } from "./AlbumCanvas";
import { ExportPreviewControl } from "./ExportPreviewControl";
import { InspectorPanel } from "./InspectorPanel";
import { MediaPanel } from "./MediaPanel";
import { ProjectCloseDialog } from "./ProjectCloseDialog";
import { useProjectCloseController } from "./useProjectCloseController";
import { useProjectEditorController } from "./useProjectEditorController";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  exportPipelinePort: ExportPipelinePort;
  projectWindowPort: ProjectWindowPort;
  runProjectMutation: ProjectMutationRunner;
  projectCorePort: ProjectCorePort;
  mediaPreviews?: Readonly<Record<string, MediaPreview>>;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onRetryUnavailableMedia(mediaId: string): Promise<void>;
  onProjectionChange(projection: EditorProjection): void;
  onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
}

export function ProjectWorkspace({
  projection,
  exportPipelinePort,
  projectWindowPort,
  runProjectMutation,
  projectCorePort,
  mediaPreviews = {},
  onMediaDemandChange,
  onRetryUnavailableMedia,
  onProjectionChange,
  onGraphicsUnavailable,
}: ProjectWorkspaceProps) {
  const [exportActive, setExportActive] = useState(false);
  const [fileMenuOpen, setFileMenuOpen] = useState(false);
  const [draggedPhotoId, setDraggedPhotoId] = useState<string | null>(null);
  const [selectedMediaId, setSelectedMediaId] = useState<string | null>(null);
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
    setSelectedMediaId(null);
  }, [projection.state.projectId]);
  useEffect(() => {
    setSelectedMediaId((current) =>
      current && projection.state.album.media.some((media) => media.id === current)
        ? current
        : null,
    );
  }, [projection.state.album.media]);
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
    projectWindowPort,
    onProjectionChange,
    onError: reportCloseError,
  });
  useEffect(() => {
    if (projectClose.interactionBlocked) {
      setFileMenuOpen(false);
    }
  }, [projectClose.interactionBlocked]);
  const controller = useProjectEditorController({
    interactionBlocked: exportActive || projectClose.interactionBlocked,
    projection,
    runProjectMutation,
    projectCorePort,
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
  const exportSheet = projection.composition.sheets.find(
    (sheet) => sheet.sheetId === controller.canvasProps.centeredSheetId,
  );
  const exportSelection = exportSheet
    ? {
        projectName: projection.state.projectName,
        sheetId: exportSheet.sheetId,
        sheetNumber: exportSheet.number,
      }
    : null;

  return (
    <div className="app-shell">
      <header className="titlebar">
        <nav className="app-menu" aria-label="Menu principal">
          <div className="app-menu-entry">
            <button
              aria-expanded={fileMenuOpen}
              aria-haspopup="menu"
              type="button"
              onClick={() => setFileMenuOpen((open) => !open)}
            >
              Arquivo
            </button>
            {fileMenuOpen && (
              <div className="app-menu-popup" role="menu">
                <button
                  disabled={projectClose.interactionBlocked}
                  role="menuitem"
                  type="button"
                  onClick={() => {
                    setFileMenuOpen(false);
                    void projectClose.requestClose();
                  }}
                >
                  Fechar Projeto
                </button>
              </div>
            )}
          </div>
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
            aria-label="Salvar"
            isDisabled={
              Boolean(busy) || exportActive || projectClose.interactionBlocked
            }
            onPress={controller.save}
          >
            ⇩
          </Button>
          <Button
            className="icon-command"
            aria-label="Desfazer"
            isDisabled={
              !projection.state.canUndo ||
              Boolean(busy) ||
              exportActive ||
              projectClose.interactionBlocked
            }
            onPress={controller.undo}
          >
            ↶
          </Button>
          <Button
            className="icon-command"
            aria-label="Refazer"
            isDisabled={
              !projection.state.canRedo ||
              Boolean(busy) ||
              exportActive ||
              projectClose.interactionBlocked
            }
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
        <ExportPreviewControl
          disabled={Boolean(busy) || projectClose.interactionBlocked}
          exportPipelinePort={exportPipelinePort}
          onActiveChange={setExportActive}
          projectId={projection.state.projectId}
          selection={exportSelection}
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
            draggedPhotoId={draggedPhotoId}
            onPhotoDragCancel={() => setDraggedPhotoId(null)}
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
          selectedMediaId={selectedMediaId}
          onMediaDemandChange={setPanelMediaDemand}
          onFillPhoto={controller.fillMedia}
          onImportPhoto={() => {
            void controller.importPhoto().then((mediaId) => {
              if (mediaId) setSelectedMediaId(mediaId);
            });
          }}
          onSelectMedia={setSelectedMediaId}
          onPhotoDragStart={setDraggedPhotoId}
          onPhotoDragEnd={() => setDraggedPhotoId(null)}
          onRelinkMedia={controller.relinkMedia}
          onRetryUnavailableMedia={onRetryUnavailableMedia}
          relinkDisabled={
            Boolean(busy) || exportActive || projectClose.interactionBlocked
          }
        />
      </div>

      {(busy || message || closeMessage) && (
        <div
          className={`operation-toast ${message || closeMessage ? "error" : ""}`}
          role={message || closeMessage ? "alert" : "status"}
        >
          {busy && <span className="toast-spinner" aria-hidden="true" />}
          <div>
            <strong>
              {message || closeMessage
                ? "A operação não foi concluída"
                : busy}
            </strong>
            <span>{closeMessage ?? message ?? "Aguarde…"}</span>
          </div>
          {!busy && (
            <button
              type="button"
              aria-label="Fechar mensagem"
              onClick={() => {
                setCloseMessage(null);
                controller.dismissFeedback();
              }}
            >
              ×
            </button>
          )}
        </div>
      )}

      {projectClose.confirmationVisible && (
        <ProjectCloseDialog
          busy={projectClose.resolving}
          onChoose={(choice) => void projectClose.resolveClose(choice)}
        />
      )}
    </div>
  );
}
