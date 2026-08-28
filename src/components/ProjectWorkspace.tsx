import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  ExportPipelinePort,
  MediaPreview,
  MediaPreviewDemand,
  ProjectCorePort,
  ProjectWindowPort,
} from "../application/projectPorts";
import {
  createFallbackWorkspacePreferencesPort,
  type WorkspacePreferencesPort,
} from "../application/workspacePreferences";
import {
  displayUnitLabel,
  formatMicrometers,
} from "../application/physicalMeasurements";
import { sheetStructureAvailability } from "../application/sheetStructure";
import type { ProjectDialogPort } from "../application/projectDialogPort";
import type { GraphicsDiagnostic } from "../application/graphics";
import { renderableMediaPreviewUrls } from "../application/mediaPreviews";
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
import { SheetContextMenu } from "./SheetContextMenu";
import {
  createSheetReorderSession,
  reduceSheetReorderSession,
  sheetReorderRepresentation,
  type SheetReorderSession,
  type SheetReorderSurface,
} from "./sheetReorderSession";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import { useWorkspacePreferences } from "../state/useWorkspacePreferences";
import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

interface ProjectWorkspaceProps {
  projection: EditorProjection;
  projectDialogPort: ProjectDialogPort;
  exportPipelinePort: ExportPipelinePort;
  projectWindowPort: ProjectWindowPort;
  runProjectMutation: ProjectMutationRunner;
  projectCorePort: ProjectCorePort;
  mediaPreviews: Readonly<Record<string, MediaPreview>>;
  onMediaDemandChange(demand: MediaPreviewDemand): void;
  onRetryUnavailableMedia(mediaId: string): Promise<void>;
  onProjectionChange(projection: EditorProjection): void;
  onGraphicsUnavailable(diagnostic: GraphicsDiagnostic): void;
  onPreferencesReady(projectId: string): void;
  workspacePreferences:
    | { kind: "persistent"; port: WorkspacePreferencesPort }
    | { kind: "memory" };
}

const SHEET_EDITING_MEDIA_PANEL_HEIGHT = 120;

export function ProjectWorkspace({
  projection,
  projectDialogPort,
  exportPipelinePort,
  projectWindowPort,
  runProjectMutation,
  projectCorePort,
  mediaPreviews,
  onMediaDemandChange,
  onRetryUnavailableMedia,
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
    setSelectedMediaId(null);
    setDraggedPhotoId(null);
  }, [projectId]);
  useEffect(() => {
    setSelectedMediaId((current) =>
      current && projection.state.album.media.some((media) => media.id === current)
        ? current
        : null,
    );
  }, [projection.state.album.media]);
  useEffect(() => {
    if (workspacePreferences.ready) onPreferencesReady(projectId);
  }, [onPreferencesReady, projectId, workspacePreferences.ready]);
  const [exportActive, setExportActive] = useState(false);
  const [saveAsBarrierActive, setSaveAsBarrierActive] = useState(false);
  const saveAsBarrierRef = useRef(false);
  const [draggedPhotoId, setDraggedPhotoId] = useState<string | null>(null);
  const [selectedMediaId, setSelectedMediaId] = useState<string | null>(null);
  const [sheetContextMenu, setSheetContextMenu] = useState<{
    position: { x: number; y: number };
    sheetId: string;
  } | null>(null);
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
    () => renderableMediaPreviewUrls(mediaPreviews),
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
  const changeSaveAsBarrier = useCallback((active: boolean) => {
    saveAsBarrierRef.current = active;
    setSaveAsBarrierActive(active);
  }, []);

  useEffect(() => {
    const rejectTerminalKeyboardInput = (event: KeyboardEvent) => {
      if (!saveAsBarrierRef.current) return;
      event.preventDefault();
      event.stopImmediatePropagation();
    };
    window.addEventListener("keydown", rejectTerminalKeyboardInput, true);
    return () =>
      window.removeEventListener("keydown", rejectTerminalKeyboardInput, true);
  }, []);
  const projectClose = useProjectCloseController({
    projectDialogPort,
    projectWindowPort,
    requestBlocked: saveAsBarrierActive,
    waitForPendingMutations: runProjectMutation.waitForIdle,
    onProjectionChange,
    onError: reportCloseError,
  });
  const controller = useProjectEditorController({
    interactionBlocked:
      exportActive || projectClose.interactionBlocked || saveAsBarrierActive,
    projection,
    runProjectMutation,
    projectCorePort,
    onProjectionChange,
    onSaveAsBarrierChange: changeSaveAsBarrier,
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
  const commandsBlocked =
    exportActive ||
    projectClose.interactionBlocked ||
    albumInformationApply.active ||
    saveAsBarrierActive;
  const sheetOrderSignature = projection.state.album.sheets
    .map((sheet) => sheet.id)
    .join(",");
  const authoritativeSheetsRef = useRef(projection.state.album.sheets);
  authoritativeSheetsRef.current = projection.state.album.sheets;
  const sheetReorderAttemptRef = useRef(0);
  const [sheetReorderSession, setSheetReorderSession] =
    useState<SheetReorderSession>(() =>
      createSheetReorderSession(projection.state.album.sheets),
    );
  const sheetReorderSessionRef = useRef(sheetReorderSession);
  sheetReorderSessionRef.current = sheetReorderSession;
  useEffect(() => {
    sheetReorderAttemptRef.current += 1;
    const next = createSheetReorderSession(projection.state.album.sheets);
    sheetReorderSessionRef.current = next;
    setSheetReorderSession(next);
  }, [projectId, sheetOrderSignature]);
  const sheetReorderDisabled =
    commandsBlocked || controller.structuralCommandsDisabled;
  useEffect(() => {
    if (sheetReorderDisabled) setSheetContextMenu(null);
  }, [sheetReorderDisabled]);
  const updateSheetReorder = useCallback((session: SheetReorderSession) => {
    sheetReorderSessionRef.current = session;
    setSheetReorderSession(session);
  }, []);
  const previewSheetReorder = useCallback(
    (
      origin: SheetReorderSurface,
      draggedSheetId: string,
      targetIndex: number,
    ) => {
      if (sheetReorderDisabled) return;
      const transition = reduceSheetReorderSession(
        sheetReorderSessionRef.current,
        projection.state.album.sheets,
        { type: "preview", origin, draggedSheetId, targetIndex },
      );
      updateSheetReorder(transition.session);
    },
    [
      projection.state.album.sheets,
      sheetReorderDisabled,
      updateSheetReorder,
    ],
  );
  const dropSheetReorder = useCallback(
    (surface: SheetReorderSurface) => {
      if (sheetReorderDisabled) return;
      const transition = reduceSheetReorderSession(
        sheetReorderSessionRef.current,
        projection.state.album.sheets,
        { type: "drop", surface },
      );
      updateSheetReorder(transition.session);
      if (!transition.effect) return;
      const attempt = sheetReorderAttemptRef.current + 1;
      sheetReorderAttemptRef.current = attempt;
      void controller
        .reorderSheet(
          transition.effect.sheetId,
          transition.effect.targetIndex,
        )
        .then((completed) => {
          if (sheetReorderAttemptRef.current !== attempt) return;
          if (completed) return;
          updateSheetReorder(
            createSheetReorderSession(authoritativeSheetsRef.current),
          );
        });
    },
    [
      controller,
      projection.state.album.sheets,
      sheetReorderDisabled,
      updateSheetReorder,
    ],
  );
  const cancelSheetReorder = useCallback(() => {
    const transition = reduceSheetReorderSession(
      sheetReorderSessionRef.current,
      projection.state.album.sheets,
      { type: "escape" },
    );
    updateSheetReorder(transition.session);
  }, [projection.state.album.sheets, updateSheetReorder]);
  useEffect(() => {
    if (
      sheetReorderSession.status !== "preview" &&
      sheetReorderSession.status !== "invalid"
    ) {
      return;
    }
    const onEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      cancelSheetReorder();
    };
    window.addEventListener("keydown", onEscape);
    return () => window.removeEventListener("keydown", onEscape);
  }, [cancelSheetReorder, sheetReorderSession.status]);

  const implicitSheetId =
    canvasMode.kind === "sheet-editing"
      ? canvasMode.sheetId
      : projection.state.album.sheets.some(
            (sheet) => sheet.id === controller.canvasProps.centeredSheetId,
          )
        ? controller.canvasProps.centeredSheetId
        : projection.state.album.sheets[0]?.id ?? null;
  const implicitSheetAvailability = sheetStructureAvailability(
    projection.state.album.sheets,
    implicitSheetId ?? "",
  );
  useProjectCommandShortcuts({
    canRedo: projection.state.canRedo,
    canUndo: projection.state.canUndo,
    closeProject: projectClose.requestClose,
    disabled: commandsBlocked,
    redo: controller.redo,
    save: controller.save,
    saveAs: controller.saveAs,
    undo: controller.undo,
  });
  const applicationMenus = createProjectApplicationMenus({
    addSheetAfter: () => {
      void controller.addSheetAfter();
    },
    addSheetBefore: () => {
      void controller.addSheetBefore();
    },
    canAddAfter: implicitSheetAvailability.canAddAfter,
    canAddBefore: implicitSheetAvailability.canAddBefore,
    canDelete: implicitSheetAvailability.canDelete,
    canExport: controller.canvasProps.centeredSheetId !== null,
    canRedo: projection.state.canRedo,
    canUndo: projection.state.canUndo,
    contextualPanelVisible: workspacePanels.panels.inspector.visible,
    closeProject: () => void projectClose.requestClose(),
    deleteSheet: () => {
      void controller.deleteSheet();
    },
    exportSheet: () => exportControlRef.current?.start(),
    mediaPanelVisible: workspacePanels.panels.media.visible,
    redo: () => void controller.redo(),
    save: () => void controller.save(),
    saveAs: () => void controller.saveAs(),
    structuralCommandsDisabled: sheetReorderDisabled,
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
          disabled={projectClose.interactionBlocked || saveAsBarrierActive}
          exportPipelinePort={exportPipelinePort}
          onActiveChange={setExportActive}
          projectId={projection.state.projectId}
          selection={exportSelection}
        />
      </div>

      <div
        aria-busy={saveAsBarrierActive || undefined}
        className="workspace-grid"
        inert={saveAsBarrierActive}
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
            draggedPhotoId={draggedPhotoId}
            onPhotoDragCancel={() => setDraggedPhotoId(null)}
            sheetReorder={{
              disabled: sheetReorderDisabled,
              representation: sheetReorderRepresentation(
                sheetReorderSession,
                "bar",
              ),
              status: sheetReorderSession.status,
              onPreview: (draggedSheetId, targetIndex) =>
                previewSheetReorder("bar", draggedSheetId, targetIndex),
              onDrop: () => dropSheetReorder("bar"),
              onCancel: cancelSheetReorder,
              onNavigate: controller.navigateToSheet,
            }}
            mediaPreviewUrls={mediaPreviewUrls}
            technicalGuides={{
              bleedUm: projection.state.document.bleedUm,
              safetyUm: projection.state.document.safetyUm,
            }}
            onMediaDemandChange={setCanvasMediaDemand}
            onGraphicsUnavailable={onGraphicsUnavailable}
            onOpenSheetContextMenu={(sheetId, position) => {
              if (sheetReorderDisabled) return;
              controller.navigateToSheet(sheetId);
              setSheetContextMenu({ position, sheetId });
            }}
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
          mediaPreviews={mediaPreviews}
          revision={projection.state.revision}
          onBeginPhotoZoom={controller.beginZoomGesture}
          onUpdatePhotoZoom={controller.updateZoomGesture}
          onFinishPhotoZoom={controller.finishZoomGesture}
          onApplyAlbumInformation={albumInformationApply.requestApply}
          onApplyAlbumDesign={controller.applyAlbumDesign}
          onPresentationUnitChange={changePresentationUnit}
          onValidateAlbumInformation={projectCorePort.validateAlbumInformation}
          onNavigateToSheet={controller.navigateToSheet}
          onOpenSheetContextMenu={(sheetId, position) => {
            if (sheetReorderDisabled) return;
            controller.navigateToSheet(sheetId);
            setSheetContextMenu({ position, sheetId });
          }}
          sheetReorder={{
            disabled: sheetReorderDisabled,
            representation: sheetReorderRepresentation(
              sheetReorderSession,
              "grid",
            ),
            status: sheetReorderSession.status,
            onPreview: (draggedSheetId, targetIndex) =>
              previewSheetReorder("grid", draggedSheetId, targetIndex),
            onDrop: () => dropSheetReorder("grid"),
            onCancel: cancelSheetReorder,
          }}
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
          onFillPhoto={controller.fillMedia}
          selectedMediaId={selectedMediaId}
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
          relinkDisabled={commandsBlocked}
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
          previewSource={{
            kind: "connected",
            previews: mediaPreviews,
            onDemandChange: setPanelMediaDemand,
          }}
        />}
      </div>

      {sheetContextMenu ? (
        <SheetContextMenu
          availability={sheetStructureAvailability(
            projection.state.album.sheets,
            sheetContextMenu.sheetId,
          )}
          position={sheetContextMenu.position}
          sheetNumber={
            projection.state.album.sheets.find(
              (sheet) => sheet.id === sheetContextMenu.sheetId,
            )?.number ?? 0
          }
          onAddAfter={() => {
            void controller.addSheetAfter(sheetContextMenu.sheetId);
          }}
          onAddBefore={() => {
            void controller.addSheetBefore(sheetContextMenu.sheetId);
          }}
          onDelete={() => {
            void controller.deleteSheet(sheetContextMenu.sheetId);
          }}
          onDismiss={() => setSheetContextMenu(null)}
        />
      ) : null}

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
