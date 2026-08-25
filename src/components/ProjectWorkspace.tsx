import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  ExportPort,
  MediaPreview,
  MediaPreviewDemand,
  ProjectWindowPort,
  ProjectSessionPort,
} from "../application/projectPorts";
import {
  createFallbackWorkspacePreferencesPort,
  type WorkspacePreferencesPort,
} from "../application/workspacePreferences";
import {
  displayUnitLabel,
  formatMicrometers,
} from "../application/physicalMeasurements";
import type { ProjectDialogPort } from "../application/projectDialogPort";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { DisplayUnit, EditorProjection } from "../domain/project";
import { ApplicationHeader } from "../ui";
import { AlbumCanvas } from "./AlbumCanvas";
import { ApplicationMenuBar } from "./ApplicationMenuBar";
import {
  ExportPreviewControl,
  type ExportPreviewControlHandle,
} from "./ExportPreviewControl";
import {
  InspectorPanel,
  type InspectorContext,
} from "./InspectorPanel";
import { MediaPanel } from "./MediaPanel";
import { createProjectApplicationMenus } from "./projectApplicationMenus";
import { useProjectCommandShortcuts } from "./useProjectCommandShortcuts";
import { useProjectCloseController } from "./useProjectCloseController";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectOperationFailureDialog } from "./useProjectOperationFailureDialog";
import { useAlbumInformationApplyController } from "./useAlbumInformationApplyController";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import { useWorkspacePreferences } from "../state/useWorkspacePreferences";
import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  projectDialogPort: ProjectDialogPort;
  exportPort: ExportPort;
  projectWindowPort: ProjectWindowPort;
  runProjectMutation: ProjectMutationRunner;
  validateAlbumInformation: ProjectSessionPort["validateAlbumInformation"];
  mediaPreviews?: Readonly<Record<string, MediaPreview>>;
  onMediaDemandChange?(demand: MediaPreviewDemand): void;
  onProjectionChange(projection: EditorProjection): void;
  onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
  onPreferencesReady?(projectId: string): void;
  workspacePreferences:
    | { kind: "persistent"; port: WorkspacePreferencesPort }
    | { kind: "memory" };
}

const SHEET_EDITING_MEDIA_PANEL_HEIGHT = 120;

export function ProjectWorkspace({
  projection,
  projectDialogPort,
  exportPort,
  projectWindowPort,
  runProjectMutation,
  validateAlbumInformation,
  mediaPreviews = {},
  onMediaDemandChange,
  onProjectionChange,
  onGraphicsUnavailable,
  onPreferencesReady,
  workspacePreferences: workspacePreferencesMode,
}: ProjectWorkspaceProps) {
  const fallbackWorkspacePreferencesPort =
    useRef<WorkspacePreferencesPort | null>(null);
  if (
    workspacePreferencesMode.kind === "memory" &&
    !fallbackWorkspacePreferencesPort.current
  ) {
    fallbackWorkspacePreferencesPort.current =
      createFallbackWorkspacePreferencesPort();
  }
  const workspacePreferencesPort =
    workspacePreferencesMode.kind === "persistent"
      ? workspacePreferencesMode.port
      : fallbackWorkspacePreferencesPort.current!;
  const workspacePreferences = useWorkspacePreferences(
    workspacePreferencesPort,
  );
  const projectId = projection.state.projectId;
  useEffect(() => {
    if (workspacePreferences.ready) onPreferencesReady?.(projectId);
  }, [onPreferencesReady, projectId, workspacePreferences.ready]);
  const [exportActive, setExportActive] = useState(false);
  const [closeMessage, setCloseMessage] = useState<string | null>(null);
  const [presentationUnitOverride, setPresentationUnitOverride] = useState<{
    projectId: string;
    unit: DisplayUnit;
  } | null>(null);
  const exportControlRef = useRef<ExportPreviewControlHandle>(null);
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
  const albumDesignPreloadMediaIds = useMemo(
    () =>
      projection.state.album.media.flatMap((media) =>
        media.kind === "decorative" ? [media.id] : [],
      ),
    [projection.state.album.media],
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
          ...albumDesignPreloadMediaIds,
        ].filter((mediaId) => !visibleSet.has(mediaId)),
      ),
    );
    onMediaDemandChange({
      visibleMediaIds: visible,
      preloadMediaIds: preload,
    });
  }, [
    albumDesignPreloadMediaIds,
    canvasMediaDemand,
    onMediaDemandChange,
    panelMediaDemand,
  ]);
  const reportCloseError = useCallback((value: string) => {
    setCloseMessage(value);
  }, []);
  const projectClose = useProjectCloseController({
    projectDialogPort,
    projectWindowPort,
    waitForPendingMutations: runProjectMutation.waitForIdle,
    onProjectionChange,
    onError: reportCloseError,
  });
  const controller = useProjectEditorController({
    interactionBlocked: exportActive || projectClose.interactionBlocked,
    projection,
    runProjectMutation,
    onProjectionChange,
  });
  const albumInformationApply = useAlbumInformationApplyController({
    projectDialogPort,
    onApply: controller.applyAlbumInformation,
    onError: setCloseMessage,
  });
  useProjectOperationFailureDialog({
    message: closeMessage ?? controller.message,
    projectDialogPort,
    onDismiss: () => {
      setCloseMessage(null);
      controller.dismissFeedback();
    },
  });
  const updateWorkspacePanelSize = useCallback(
    (panel: "inspector" | "media", size: number) => {
      workspacePreferences.update({
        kind: "workspacePanelSize",
        panel,
        size,
      });
    },
    [workspacePreferences.update],
  );
  const updateWorkspacePanelVisibility = useCallback(
    (panel: "inspector" | "media", visible: boolean) => {
      workspacePreferences.update({
        kind: "workspacePanelVisibility",
        panel,
        visible,
      });
    },
    [workspacePreferences.update],
  );
  const workspacePanels = useWorkspacePanelLayout({
    preferences: workspacePreferences.preferences.workspacePanels,
    onSizeChange: updateWorkspacePanelSize,
    onVisibilityChange: updateWorkspacePanelVisibility,
  });
  const presentationUnit =
    presentationUnitOverride?.projectId === projectId
      ? presentationUnitOverride.unit
      : projection.state.document.displayUnit;
  const changePresentationUnit = useCallback(
    (unit: DisplayUnit | null) => {
      setPresentationUnitOverride((current) => {
        if (unit !== null) return { projectId, unit };
        return current?.projectId === projectId ? null : current;
      });
    },
    [projectId],
  );
  const sheetEditing = controller.canvasProps.mode.kind === "sheet-editing";
  const mediaPanelHeight = sheetEditing
    ? SHEET_EDITING_MEDIA_PANEL_HEIGHT
    : workspacePanels.panels.media.size;
  const workspaceStyle = {
    ...workspacePanels.style,
    "--media-panel-height": workspacePanels.panels.media.visible
      ? `${mediaPanelHeight}px`
      : "0px",
  };
  const {
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom,
    displayedPhotoPanX,
  } = controller;
  const canvasMode = controller.canvasProps.mode;
  const editingSheet =
    canvasMode.kind === "sheet-editing"
      ? projection.composition.sheets.find(
          (sheet) => sheet.sheetId === canvasMode.sheetId,
        ) ?? null
      : null;
  const inspectorContext: InspectorContext = selectedFrame
    ? {
        kind: "frame",
        frame: selectedFrame,
        composedPhoto: selectedComposedPhoto,
        ...(editingSheet ? { editingSheet } : {}),
      }
    : editingSheet
      ? { kind: "sheet", sheet: editingSheet }
      : { kind: "album" };
  const projectMetadata = projectAlbumMetadata(projection, presentationUnit);
  const commandsBlocked =
    exportActive ||
    projectClose.interactionBlocked ||
    albumInformationApply.active;
  useProjectCommandShortcuts({
    canRedo: projection.state.canRedo,
    canUndo: projection.state.canUndo,
    closeProject: projectClose.requestClose,
    disabled: commandsBlocked,
    redo: controller.redo,
    save: controller.save,
    undo: controller.undo,
  });
  const applicationMenus = createProjectApplicationMenus({
    canExport: controller.canvasProps.centeredSheetId !== null,
    canRedo: projection.state.canRedo,
    canUndo: projection.state.canUndo,
    contextualPanelVisible: workspacePanels.panels.inspector.visible,
    closeProject: () => void projectClose.requestClose(),
    exportSheet: () => exportControlRef.current?.start(),
    mediaPanelVisible: workspacePanels.panels.media.visible,
    redo: () => void controller.redo(),
    save: () => void controller.save(),
    undo: () => void controller.undo(),
    toggleContextualPanel: () =>
      workspacePanels.setPanelVisibility(
        "inspector",
        !workspacePanels.panels.inspector.visible,
      ),
    toggleMediaPanel: () =>
      workspacePanels.setPanelVisibility(
        "media",
        !workspacePanels.panels.media.visible,
      ),
  });

  return (
    <div className="app-shell">
      <ApplicationHeader
        context={projection.state.projectName}
        metadata={projectMetadata}
        status={projection.state.dirty ? "alterações não salvas" : "salvo"}
      />

      <div className="commandbar">
        <ApplicationMenuBar
          disabled={commandsBlocked}
          groups={applicationMenus}
        />
        <ExportPreviewControl
          ref={exportControlRef}
          dialogPort={projectDialogPort}
          disabled={projectClose.interactionBlocked}
          exportPort={exportPort}
          onActiveChange={setExportActive}
          projectId={projection.state.projectId}
          sheetId={controller.canvasProps.centeredSheetId}
        />
      </div>

      <div
        className="workspace-grid"
        ref={workspacePanels.workspaceRef}
        style={workspaceStyle}
      >
        <section
          id="continuous-canvas"
          className="canvas-section"
          aria-label="Área de composição"
        >
          <AlbumCanvas
            {...controller.canvasProps}
            mediaPreviewUrls={mediaPreviewUrls}
            technicalGuides={{
              bleedUm: projection.state.document.bleedUm,
              safetyUm: projection.state.document.safetyUm,
            }}
            onMediaDemandChange={setCanvasMediaDemand}
            onGraphicsUnavailable={onGraphicsUnavailable}
          />
        </section>

        {workspacePanels.panels.media.visible && (
          <WorkspacePanelSplitter
            disabled={sheetEditing}
            panel="media"
            size={mediaPanelHeight}
            onResizeStart={workspacePanels.beginResize}
            onResizeBy={workspacePanels.resizeBy}
          />
        )}

        {workspacePanels.panels.inspector.visible && (
          <WorkspacePanelSplitter
            panel="inspector"
            size={workspacePanels.panels.inspector.size}
            onResizeStart={workspacePanels.beginResize}
            onResizeBy={workspacePanels.resizeBy}
          />
        )}

        {workspacePanels.panels.inspector.visible && <InspectorPanel
          key={projectId}
          context={inspectorContext}
          displayedPhotoZoom={displayedPhotoZoom}
          displayedPhotoPanX={displayedPhotoPanX}
          zoomCommitting={controller.zoomCommitting}
          document={projection.state.document}
          presentationUnit={presentationUnit}
          mediaItems={projection.state.album.media}
          sheetStates={projection.state.album.sheets}
          sheets={projection.composition.sheets}
          frameBorder={projection.composition.frameBorder}
          visualDefaults={projection.state.album.visualDefaults}
          focusedSheetId={controller.canvasProps.focusedSheetId}
          mediaPreviewUrls={mediaPreviewUrls}
          revision={projection.state.revision}
          onBeginPhotoZoom={controller.beginZoomGesture}
          onUpdatePhotoZoom={controller.updateZoomGesture}
          onFinishPhotoZoom={controller.finishZoomGesture}
          onApplyAlbumInformation={albumInformationApply.requestApply}
          onApplyAlbumDesign={controller.applyAlbumDesign}
          onPresentationUnitChange={changePresentationUnit}
          onValidateAlbumInformation={validateAlbumInformation}
          onNavigateToSheet={controller.navigateToSheet}
          sectionState={{
            kind: "controlled",
            values: workspacePreferences.preferences.inspectorSections,
            onChange: (preferenceKey, open) =>
              workspacePreferences.update({
                kind: "inspectorSection",
                preferenceKey,
                open,
              }),
          }}
        />}

        {workspacePanels.panels.media.visible && <MediaPanel
          mediaItems={projection.state.album.media}
          mediaUsage={projection.mediaUsage}
          mediaPreviews={mediaPreviews}
          onMediaDemandChange={setPanelMediaDemand}
          onFillPhoto={controller.fillMedia}
          preferences={{
            kind: "controlled",
            persistent: workspacePreferences.preferences.mediaPanel,
            thumbnailSizes:
              workspacePreferences.preferences.mediaThumbnailSizes,
            onThumbnailSizeChange: (mediaKind, size) =>
              workspacePreferences.update({
                kind: "mediaThumbnailSize",
                mediaKind,
                size,
              }),
            onSortDirectionChange: (mediaKind, sortDirection) =>
              workspacePreferences.update({
                kind: "mediaPanelSortDirection",
                mediaKind,
                sortDirection,
              }),
            onUsageFilterChange: (mediaKind, usageFilter) =>
              workspacePreferences.update({
                kind: "mediaPanelUsageFilter",
                mediaKind,
                usageFilter,
              }),
          }}
        />}
      </div>

    </div>
  );
}

function projectAlbumMetadata(
  projection: EditorProjection,
  presentationUnit: DisplayUnit,
) {
  const { album, document } = projection.state;
  const width = formatMicrometers(document.sheetWidthUm / 2, presentationUnit);
  const height = formatMicrometers(document.sheetHeightUm, presentationUnit);
  const sheetLabel = album.sheets.length === 1 ? "Lâmina" : "Lâminas";
  return `${width}×${height} ${displayUnitLabel(presentationUnit)} · ${album.sheets.length} ${sheetLabel}`;
}
