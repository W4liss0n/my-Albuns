import { useState } from "react";
import { Button } from "react-aria-components";

import type { ExportPort } from "../application/projectPorts";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { EditorProjection } from "../domain/project";
import { AlbumCanvas } from "./AlbumCanvas";
import { ExportPreviewControl } from "./ExportPreviewControl";
import { InspectorPanel } from "./InspectorPanel";
import { MediaPanel } from "./MediaPanel";
import { useProjectEditorController } from "./useProjectEditorController";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  exportPort: ExportPort;
  runProjectMutation: ProjectMutationRunner;
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  onProjectionChange(projection: EditorProjection): void;
  onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
}

export function ProjectWorkspace({
  projection,
  exportPort,
  runProjectMutation,
  mediaPreviewUrls = {},
  onProjectionChange,
  onGraphicsUnavailable,
}: ProjectWorkspaceProps) {
  const [exportActive, setExportActive] = useState(false);
  const controller = useProjectEditorController({
    interactionBlocked: exportActive,
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
            isDisabled={
              !projection.state.canUndo || Boolean(busy) || exportActive
            }
            onPress={controller.undo}
          >
            ↶
          </Button>
          <Button
            className="icon-command"
            aria-label="Refazer"
            isDisabled={
              !projection.state.canRedo || Boolean(busy) || exportActive
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
          disabled={Boolean(busy)}
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
          onNavigateToSheet={controller.navigateToSheet}
        />

        <MediaPanel
          mediaItems={projection.state.album.media}
          mediaUsage={projection.mediaUsage}
          mediaPreviewUrls={mediaPreviewUrls}
          onFillPhoto={controller.fillMedia}
        />
      </div>

      {(busy || message) && (
        <div
          className={`operation-toast ${message ? "error" : ""}`}
          role={message ? "alert" : "status"}
        >
          {busy && <span className="toast-spinner" aria-hidden="true" />}
          <div>
            <strong>
              {message ? "A operação não foi concluída" : busy}
            </strong>
            <span>{message ?? "Aguarde…"}</span>
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
