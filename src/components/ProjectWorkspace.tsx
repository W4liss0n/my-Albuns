import { useCallback, useEffect, useState } from "react";
import { Button } from "react-aria-components";

import type {
  ExportPort,
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
  exportPort: ExportPort;
  projectWindowPort: ProjectWindowPort;
  runProjectMutation: ProjectMutationRunner;
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  onProjectionChange(projection: EditorProjection): void;
  onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
}

export function ProjectWorkspace({
  projection,
  exportPort,
  projectWindowPort,
  runProjectMutation,
  mediaPreviewUrls = {},
  onProjectionChange,
  onGraphicsUnavailable,
}: ProjectWorkspaceProps) {
  const [exportActive, setExportActive] = useState(false);
  const [fileMenuOpen, setFileMenuOpen] = useState(false);
  const [closeMessage, setCloseMessage] = useState<string | null>(null);
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
          exportPort={exportPort}
          onActiveChange={setExportActive}
          projectId={projection.state.projectId}
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
          mediaPreviewUrls={mediaPreviewUrls}
          onFillPhoto={controller.fillMedia}
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
