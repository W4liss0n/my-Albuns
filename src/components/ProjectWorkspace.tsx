import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";

import type {
  ExportPipelinePort,
  MediaPreview,
  MediaFileInfo,
  ImageProcessingProgress,
  ImageProcessingProblem,
  MediaImportCompletion,
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
import type { ImageViewerWindowPort, ViewerPresentation } from "../application/imageViewerWindow";
import type { GraphicsDiagnostic } from "../application/graphics";
import { mergeMediaPreviewDemands, renderableMediaPreviewUrls } from "../application/mediaPreviews";
import type { DisplayUnit, EditorProjection } from "../domain/project";
import { ApplicationHeader } from "../ui";
import { AlbumCanvas } from "./AlbumCanvas";
import { LayoutPanel } from "./LayoutPanel";
import { LayoutCatalogNotice } from "./LayoutCatalogNotice";
import { ApplicationMenuBar } from "./ApplicationMenuBar";
import {
  ExportPreviewControl,
  type ExportPreviewControlHandle,
} from "./ExportPreviewControl";
import {
  InspectorPanel,
  type InspectorContext,
} from "./InspectorPanel";
import { MediaPanel, type MediaPanelHandle } from "./MediaPanel";
import { adjacentViewerDemand, sheetViewerMediaIds, viewerPreviewState } from "./imageViewerModel";
import { ownsEditingKeys } from "./keyboardEventOwnership";
import { createProjectApplicationMenus } from "./projectApplicationMenus";
import { useProjectLauncher } from "./useProjectLauncher";
import { useProjectCommandShortcuts } from "./useProjectCommandShortcuts";
import { useProjectCloseController } from "./useProjectCloseController";
import { useProjectEditorController } from "./useProjectEditorController";
import { useMediaRemoval } from "./useMediaRemoval";
import { useProjectGraphicsFailureDialog } from "./useProjectGraphicsFailureDialog";
import { useProjectOperationResultDialog } from "./useProjectOperationResultDialog";
import { useImageProcessingProgressDialog } from "./useImageProcessingProgressDialog";
import { useAlbumInformationApplyController } from "./useAlbumInformationApplyController";
import { SheetContextMenu } from "./SheetContextMenu";
import { FrameContextMenu } from "./FrameContextMenu";
import { ContextMenuSurface } from "../ui/ContextMenuSurface";
import { MenuItem } from "../ui/MenuItem";
import { matchProjectCommandShortcut, projectCommandDescriptor } from "../application/projectCommandCatalog";
import {
  createSheetReorderSession,
  reduceSheetReorderSession,
  sheetReorderRepresentation,
  type SheetReorderSession,
  type SheetReorderSurface,
} from "./sheetReorderSession";
import { openProjectGeneration } from "./openProjectGeneration";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import { useWorkspacePreferences } from "../state/useWorkspacePreferences";
import { usePhotoshop } from "../state/usePhotoshop";
import "./PhotoshopNotice.css";
import { InlineNotice } from "../ui/InlineNotice";
import { ActionButton } from "../ui/ActionButton";
import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

interface ProjectWorkspaceProps {
  exportMediaPort?: import("../application/exportMedia").ExportMediaPort;
  photoshopPort?: import("../application/photoshop").PhotoshopPort;
  projectLauncher?: import("../application/projectLauncher").ProjectLauncher;
  generationLauncher?: import("../application/projectGeneration").ProjectGenerationLauncher;
  mediaDropPort?: import("../application/projectPorts").MediaDropPort;
  projection: EditorProjection;
  projectDialogPort: ProjectDialogPort;
  exportPipelinePort: ExportPipelinePort;
  projectWindowPort: ProjectWindowPort;
  imageViewerWindowPort?: ImageViewerWindowPort;
  runProjectMutation: ProjectMutationRunner;
  projectCorePort: ProjectCorePort;
  mediaPreviews: Readonly<Record<string, MediaPreview>>;
  mediaFiles?: Readonly<Record<string, MediaFileInfo>>;
  onMediaDemandChange(demand: MediaPreviewDemand): void;
  prepareMediaPresentation?(imported: MediaImportCompletion, demand: MediaPreviewDemand): Promise<readonly ImageProcessingProblem[]>;
  onRetryUnavailableMedia(mediaId: string, onProgress: (progress: ImageProcessingProgress) => void): Promise<void>;
  onProjectionChange(projection: EditorProjection): void;
  onGraphicsUnavailable(diagnostic: GraphicsDiagnostic): void;
  graphicsFailure?: Extract<GraphicsDiagnostic, { supported: false }> | null;
  onPreferencesReady(projectId: string): void;
  workspacePreferences:
    | { kind: "persistent"; port: WorkspacePreferencesPort }
    | { kind: "memory" };
}

const SHEET_EDITING_MEDIA_PANEL_HEIGHT = 120;

export function ProjectWorkspace({
  projectLauncher,
  generationLauncher,
  exportMediaPort,
  photoshopPort,
  mediaDropPort,
  projection,
  projectDialogPort,
  exportPipelinePort,
  projectWindowPort,
  imageViewerWindowPort,
  runProjectMutation,
  projectCorePort,
  mediaPreviews,
  mediaFiles,
  onMediaDemandChange,
  prepareMediaPresentation,
  onRetryUnavailableMedia,
  onProjectionChange,
  onGraphicsUnavailable,
  graphicsFailure = null,
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
  const [viewer, setViewer] = useState<{
    sessionId: string;
    projectId: string;
    source: "panel" | "sheet";
    mediaId: string;
    mediaIds: readonly string[];
    restoreFocus: HTMLElement | null;
    correction?: { phase: "browse" | "select" | "processing" | "preview" | "applying"; referenceIds: readonly string[]; referenceMediaId: string; token?: string; resultUrl?: string; error?: string };
  } | null>(null);
  const viewerStateRef = useRef(viewer);
  viewerStateRef.current = viewer;
  const correctionRequest = useRef(0);
  const correctionCancellation = useRef<Promise<void>>(Promise.resolve());
  const openedViewerSession = useRef<string | null>(null);
  const viewerRevision = useRef(0);
  const viewerPendingRef = useRef(false);
  const spaceCandidate = useRef<{ source: "panel" | "sheet"; mediaId: string; mediaIds: readonly string[]; focus: HTMLElement | null; cancelled: boolean } | null>(null);
  useEffect(() => {
    correctionRequest.current++;
    setMediaSelectionRequest(null);
    setMediaDrag(null);
    setViewer(null);
    viewerPendingRef.current = false;
    spaceCandidate.current = null;
  }, [projectId]);
  useEffect(() => {
    if (workspacePreferences.ready) onPreferencesReady(projectId);
  }, [onPreferencesReady, projectId, workspacePreferences.ready]);
  const [exportActive, setExportActive] = useState(false);
  const [sessionBarrierActive, setSessionBarrierActive] = useState(false);
  const sessionBarrierRef = useRef(false);
  const [mediaDrag, setMediaDrag] = useState<import("./useMediaDragGesture").MediaDrag | null>(null);
  const draggedPhotoId = mediaDrag?.kind === "photo" ? mediaDrag.mediaId : null;
  const [mediaSelectionRequest, setMediaSelectionRequest] = useState<{ mediaId: string } | null>(null);
  const [sheetContextMenu, setSheetContextMenu] = useState<{
    position: { x: number; y: number };
    sheetId: string;
  } | null>(null);
  const [closeMessage, setCloseMessage] = useState<string | null>(null);
  const [frameContextMenu, setFrameContextMenu] = useState<{
    kind: "frames" | "empty" | "photo";
    position: { x: number; y: number };
    mediaId?: string;
    sheetId?: string;
  } | null>(null);
  const photoshop = usePhotoshop(photoshopPort);
  const [presentationUnitOverride, setPresentationUnitOverride] = useState<{
    projectId: string;
    unit: DisplayUnit;
  } | null>(null);
  const exportControlRef = useRef<ExportPreviewControlHandle>(null);
  const mediaPanelRef = useRef<MediaPanelHandle>(null);
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
  const missingMediaIds = useMemo(() => new Set(
    projection.state.album.media.filter((media) =>
      (mediaFiles?.[media.id]?.state ?? mediaPreviews[media.id]?.state) === "absent"
      && !mediaPreviewUrls[media.id],
    ).map((media) => media.id),
  ), [projection.state.album.media, mediaFiles, mediaPreviews, mediaPreviewUrls]);
  const albumDesignPreloadMediaIds = useMemo(
    () =>
      projection.state.album.media.flatMap((media) =>
        media.kind === "decorative" ? [media.id] : [],
      ),
    [projection.state.album.media],
  );
  useEffect(() => {
    onMediaDemandChange(mergeMediaPreviewDemands(
      canvasMediaDemand, panelMediaDemand,
      { visibleMediaIds: [], preloadMediaIds: albumDesignPreloadMediaIds },
      viewer?.projectId === projectId ? viewer.correction
        ? { visibleMediaIds: [viewer.mediaId, viewer.correction.referenceMediaId], preloadMediaIds: [] }
        : adjacentViewerDemand(viewer.mediaIds, viewer.mediaId) : { visibleMediaIds: [], preloadMediaIds: [] },
    ));
  }, [
    albumDesignPreloadMediaIds,
    canvasMediaDemand,
    onMediaDemandChange,
    panelMediaDemand,
    viewer,
    projectId,
  ]);
  const reportCloseError = useCallback((value: string) => {
    setCloseMessage(value);
  }, []);
  const mediaRemoval = useMediaRemoval({ projection, runner: runProjectMutation, dialogPort: projectDialogPort,
    onProjectionChange, onError: reportCloseError });
  const changeSessionBarrier = useCallback((active: boolean) => {
    sessionBarrierRef.current = active;
    setSessionBarrierActive(active);
  }, []);

  useEffect(() => {
    const rejectTerminalKeyboardInput = (event: KeyboardEvent) => {
      if (!sessionBarrierRef.current && !viewerPendingRef.current) return;
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
    requestBlocked: sessionBarrierActive || mediaRemoval.active,
    waitForPendingMutations: runProjectMutation.waitForIdle,
    onProjectionChange,
    onError: reportCloseError,
  });
  useProjectGraphicsFailureDialog({
    closeCancelRevision: projectClose.explicitCancelRevision,
    diagnostic: graphicsFailure,
    onCloseProject: projectClose.requestClose,
    projectDialogPort,
  });
  const controller = useProjectEditorController({
    projectDialogPort,
    interactionBlocked:
      mediaRemoval.active ||
      exportActive ||
      projectClose.interactionBlocked ||
      sessionBarrierActive ||
      graphicsFailure !== null ||
      viewer?.projectId === projectId,
    projection,
    runProjectMutation,
    projectCorePort,
    onProjectionChange,
    onSaveAsBarrierChange: changeSessionBarrier,
    prepareImportedMedia: prepareMediaPresentation ? async (imported) => {
      const plan = mediaPanelRef.current?.planCatalog(imported.projection.state.album.media, imported.projection.mediaUsage, imported.projection.state.album.mediaFolders);
      const demand = mergeMediaPreviewDemands(
        canvasMediaDemand,
        plan?.demand ?? { visibleMediaIds: [], preloadMediaIds: [] },
        { visibleMediaIds: [], preloadMediaIds: albumDesignPreloadMediaIds },
      );
      const problems = await prepareMediaPresentation(imported, demand);
      plan?.commit();
      return problems;
    } : undefined,
  });
  const albumInformationApply = useAlbumInformationApplyController({
    projectDialogPort,
    onApply: controller.applyAlbumInformation,
    onError: setCloseMessage,
  });
  useImageProcessingProgressDialog(controller.imageProcessingProgress, projectDialogPort);
  useProjectOperationResultDialog({
    importResult: controller.photoImportResult,
    processingProblems: controller.imageProcessingProgress ? undefined : controller.imageProcessingProblems,
    processingOperationProblem: controller.imageProcessingProgress ? undefined : controller.imageProcessingOperationProblem,
    message: closeMessage ?? controller.message,
    projectDialogPort,
    onDismiss: (kind) => {
      if (kind === "imageProcessingProblems") {
        controller.dismissPhotoImportResult();
        controller.dismissImageProcessingProblems();
      } else {
        setCloseMessage(null);
        controller.dismissFeedback();
      }
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
  const mediaPanelVisible = workspacePanels.panels.media.visible && !controller.layoutPanel.visible;
  const mediaPanelHeight = sheetEditing
    ? SHEET_EDITING_MEDIA_PANEL_HEIGHT
    : workspacePanels.panels.media.size;
  const workspaceStyle = {
    ...workspacePanels.style,
    ...(!mediaPanelVisible ? { "--media-splitter-size": "0px" } : {}),
    "--media-panel-height": mediaPanelVisible
      ? `${mediaPanelHeight}px`
      : "0px",
  };
  const {
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom,
  } = controller;
  const canvasMode = controller.canvasProps.mode;
  const editingSheet =
    canvasMode.kind === "sheet-editing"
      ? projection.composition.sheets.find(
          (sheet) => sheet.sheetId === canvasMode.sheetId,
        ) ?? null
      : null;
  const inspectorContext: InspectorContext = editingSheet && controller.selectedFrames.length > 1
    ? { kind: "multiple-frames", frames: controller.selectedFrames, editingSheet }
    : selectedFrame
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
    mediaRemoval.active ||
    exportActive ||
    projectClose.interactionBlocked ||
    albumInformationApply.active ||
    sessionBarrierActive ||
    graphicsFailure !== null ||
    viewer?.projectId === projectId;
  const selectedPhotoFrame = controller.selectedFrames.length === 1 && controller.selectedFrames[0].photo
    ? controller.selectedFrames[0] : null;
  const viewerActive = viewer?.projectId === projectId ? viewer : null;
  const openViewer = (source: "panel" | "sheet", mediaId: string, mediaIds: readonly string[], focus: HTMLElement | null) => {
    if (!imageViewerWindowPort || commandsBlocked || mediaDrag || !mediaIds.includes(mediaId)) return;
    setFrameContextMenu(null);
    setSheetContextMenu(null);
    viewerPendingRef.current = true;
    setViewer({ sessionId: crypto.randomUUID(), projectId, source, mediaId, mediaIds: [...mediaIds], restoreFocus: focus });
  };
  const viewerPresentation = useMemo<ViewerPresentation | null>(() => {
    if (!viewerActive) return null;
    const position = viewerActive.mediaIds.indexOf(viewerActive.mediaId);
    const media = projection.state.album.media.find((item) => item.id === viewerActive.mediaId);
    const preview = mediaPreviews[viewerActive.mediaId];
    return {
      sessionId: viewerActive.sessionId, revision: ++viewerRevision.current,
      mediaId: viewerActive.mediaId, name: media?.name ?? "Imagem",
      url: mediaPreviewUrls[viewerActive.mediaId] ?? null,
      state: viewerPreviewState(mediaFiles?.[viewerActive.mediaId], preview),
      canPrevious: position > 0, canNext: position >= 0 && position < viewerActive.mediaIds.length - 1,
      correction: viewerActive.correction ? (() => {
        const referenceId = viewerActive.correction!.referenceMediaId;
        const refIndex = viewerActive.correction!.referenceIds.indexOf(referenceId);
        const reference = projection.state.album.media.find((item) => item.id === referenceId);
        return {
          phase: viewerActive.correction!.phase,
          referenceMediaId: referenceId, referenceName: reference?.name ?? "Imagem",
          referenceUrl: mediaPreviewUrls[referenceId] ?? null,
          referenceState: viewerPreviewState(mediaFiles?.[referenceId], mediaPreviews[referenceId]),
          canPreviousReference: refIndex > 0,
          canNextReference: refIndex < viewerActive.correction!.referenceIds.length - 1,
          resultUrl: viewerActive.correction!.resultUrl ?? null,
          error: viewerActive.correction!.error ?? null,
        };
      })() : undefined,
    };
  }, [viewerActive, projection.state.album.media, mediaPreviews, mediaPreviewUrls, mediaFiles]);
  useEffect(() => {
    if (!imageViewerWindowPort) return;
    if (!viewerPresentation) {
      const session = openedViewerSession.current;
      openedViewerSession.current = null;
      if (session) void imageViewerWindowPort.close(session).catch(() => undefined);
      return;
    }
    if (openedViewerSession.current !== viewerPresentation.sessionId) {
      const previousSession = openedViewerSession.current;
      openedViewerSession.current = viewerPresentation.sessionId;
      void (async () => {
        if (previousSession) await imageViewerWindowPort.close(previousSession);
        await imageViewerWindowPort.open(viewerPresentation);
      })().catch(() => { viewerPendingRef.current = false; setViewer((current) => {
        if (current?.sessionId !== viewerPresentation.sessionId) return current;
        requestAnimationFrame(() => (current.restoreFocus?.isConnected ? current.restoreFocus : document.querySelector<HTMLElement>(current.source === "panel" ? "#media-panel" : ".canvas-host canvas"))?.focus({ preventScroll: true }));
        return null;
      }); });
    } else {
      void imageViewerWindowPort.update(viewerPresentation).catch(() => undefined);
    }
  }, [imageViewerWindowPort, viewerPresentation]);
  useEffect(() => {
    if (!imageViewerWindowPort) return;
    let active = true;
    let stopNavigate: (() => void) | undefined;
    let stopClosed: (() => void) | undefined;
    void imageViewerWindowPort.onNavigate(({ sessionId, offset }) => {
      if (!active || (offset !== -1 && offset !== 1)) return;
      const correction = viewerStateRef.current?.correction;
      if (correction?.phase === "applying" && viewerStateRef.current?.sessionId === sessionId) return;
      if (correction && viewerStateRef.current?.sessionId === sessionId) {
        correctionRequest.current++;
        correctionCancellation.current = imageViewerWindowPort.cancelCorrection?.().catch(() => undefined) ?? Promise.resolve();
      }
      setViewer((current) => {
        if (!current || current.sessionId !== sessionId) return current;
        if (current.correction) {
          const index = current.correction.referenceIds.indexOf(current.correction.referenceMediaId);
          const referenceMediaId = current.correction.referenceIds[index + offset];
          return referenceMediaId ? { ...current, correction: { phase: "browse", referenceIds: current.correction.referenceIds, referenceMediaId } } : current;
        }
        const index = current.mediaIds.indexOf(current.mediaId);
        const mediaId = current.mediaIds[index + offset];
        return mediaId ? { ...current, mediaId } : current;
      });
    }).then((stop) => { if (active) stopNavigate = stop; else stop(); });
    void imageViewerWindowPort.onClosed((sessionId) => {
      if (!active) return;
      correctionRequest.current++;
      setViewer((current) => {
        if (current?.sessionId !== sessionId) return current;
        viewerPendingRef.current = false;
        requestAnimationFrame(() => (current.restoreFocus?.isConnected ? current.restoreFocus : document.querySelector<HTMLElement>(current.source === "panel" ? "#media-panel" : ".canvas-host canvas"))?.focus({ preventScroll: true }));
        return null;
      });
    }).then((stop) => { if (active) stopClosed = stop; else stop(); });
    return () => { active = false; stopNavigate?.(); stopClosed?.(); };
  }, [imageViewerWindowPort]);
  useEffect(() => {
    if (!imageViewerWindowPort?.onCorrection) return;
    let active = true;
    let stop: (() => void) | undefined;
    void imageViewerWindowPort.onCorrection((action) => {
      if (!active) return;
      const current = viewerStateRef.current;
      if (!current || current.sessionId !== action.sessionId || current.correction?.phase === "applying") return;
      if (action.kind === "start") {
        correctionRequest.current++;
        const referenceIds = projection.state.album.media
          .filter((media) => media.kind === "photo" && media.id !== current.mediaId)
          .map((media) => media.id);
        setViewer((value) => value?.sessionId === action.sessionId
          ? { ...value, correction: { phase: "browse", referenceIds, referenceMediaId: referenceIds[0] ?? "", error: referenceIds.length ? undefined : "Adicione outra foto ao projeto para usar como referência." } }
          : value);
        return;
      }
      const correction = current.correction;
      if (!correction) return;
      if (action.kind === "cancel") {
        correctionRequest.current++;
        correctionCancellation.current = imageViewerWindowPort.cancelCorrection?.().catch(() => undefined) ?? Promise.resolve();
        setViewer((value) => value?.sessionId === action.sessionId ? { ...value, correction: undefined } : value);
      } else if (action.kind === "browse") {
        correctionRequest.current++;
        correctionCancellation.current = imageViewerWindowPort.cancelCorrection?.().catch(() => undefined) ?? Promise.resolve();
        setViewer((value) => value?.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { phase: "browse", referenceIds: value.correction.referenceIds, referenceMediaId: value.correction.referenceMediaId } } : value);
      } else if (action.kind === "select") {
        correctionRequest.current++;
        setViewer((value) => value?.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { ...value.correction, phase: "select", error: undefined } } : value);
      } else if (action.kind === "preview" && action.targetFace && action.referenceFace
        && action.referenceMediaId === correction.referenceMediaId && imageViewerWindowPort.prepareCorrection) {
        const request = ++correctionRequest.current;
        setViewer((value) => value?.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { ...value.correction, phase: "processing", error: undefined } } : value);
        void correctionCancellation.current.then(() => {
          if (correctionRequest.current !== request) return null;
          return imageViewerWindowPort.prepareCorrection!({
            sessionId: action.sessionId, targetMediaId: current.mediaId,
            referenceMediaId: correction.referenceMediaId,
            targetFace: action.targetFace!, referenceFace: action.referenceFace!,
          });
        }).then((prepared) => {
          if (!prepared || correctionRequest.current !== request) return;
          const { token, url } = prepared;
          setViewer((value) => value?.sessionId === action.sessionId
            && value.mediaId === current.mediaId
            && value.correction?.referenceMediaId === correction.referenceMediaId
            && value.correction.phase === "processing"
            ? { ...value, correction: { ...value.correction, phase: "preview", token, resultUrl: url } } : value);
        }).catch((error) => {
          if (correctionRequest.current !== request) return;
          setViewer((value) => value?.sessionId === action.sessionId && value.mediaId === current.mediaId
            && value.correction?.referenceMediaId === correction.referenceMediaId && value.correction?.phase === "processing"
            ? { ...value, correction: { ...value.correction, phase: "select", error: error instanceof Error ? error.message : "Não foi possível corrigir os olhos." } } : value);
        });
      } else if (action.kind === "apply" && correction.phase === "preview" && correction.token && imageViewerWindowPort.applyCorrection) {
        const token = correction.token;
        setViewer((value) => value?.sessionId === action.sessionId && value.correction
          ? { ...value, correction: { ...value.correction, phase: "applying", error: undefined } } : value);
        void runProjectMutation.run(() => imageViewerWindowPort.applyCorrection!(action.sessionId, token)).then((outcome) => {
          if (outcome.status === "completed") {
            onProjectionChange(outcome.projection);
            setViewer((value) => value?.sessionId === action.sessionId ? { ...value, correction: undefined } : value);
          } else if (outcome.status === "failed") {
            setViewer((value) => value?.sessionId === action.sessionId && value.correction
              ? { ...value, correction: { ...value.correction, phase: "preview", error: outcome.error instanceof Error ? outcome.error.message : "Não foi possível aplicar a correção." } } : value);
          }
        });
      }
    }).then((value) => { if (active) stop = value; else value(); });
    return () => { active = false; stop?.(); };
  }, [imageViewerWindowPort, projection.state.album.media, runProjectMutation, onProjectionChange]);  useEffect(() => {
    const keyDown = (event: globalThis.KeyboardEvent) => {
      if (viewerActive) return;
      if (event.code !== "Space") {
        if (spaceCandidate.current) spaceCandidate.current.cancelled = true;
        return;
      }
      if (event.repeat || ownsEditingKeys(event.target) || commandsBlocked || mediaDrag || frameContextMenu || sheetContextMenu) return;
      if (spaceCandidate.current) return;
      const target = event.target instanceof Element ? event.target : null;
      const panel = target?.closest("#media-panel");
      if (panel) {
        const card = target?.closest<HTMLElement>("[data-media-id]");
        if (target?.closest("button, [role=menu], [role=menubar]") && !card) return;
        if (matchProjectCommandShortcut(event, "media-photo") !== "view-image") return;
        const selection = mediaPanelRef.current?.viewerSelection(card?.dataset.mediaId);
        if (!selection) return;
        spaceCandidate.current = { source: "panel", ...selection, focus: target instanceof HTMLElement ? target : null, cancelled: false };
      } else if (target?.closest(".canvas-host") && !target.closest("button, [role=menu], [role=menubar]")) {
        if (matchProjectCommandShortcut(event, "frame-photo") !== "view-image") return;
        const frame = selectedPhotoFrame;
        if (!frame?.photo) return;
        const sheet = projection.composition.sheets.find((item) => item.frames.some((candidate) => candidate.frameId === frame.id)) ?? null;
        const mediaIds = sheetViewerMediaIds(sheet);
        if (!mediaIds.includes(frame.photo.mediaId)) return;
        spaceCandidate.current = { source: "sheet", mediaId: frame.photo.mediaId, mediaIds, focus: target instanceof HTMLElement ? target : null, cancelled: false };
      } else return;
      if (panel) event.preventDefault();
    };
    const keyUp = (event: globalThis.KeyboardEvent) => {
      if (viewerActive) return;
      if (event.code !== "Space") return;
      const candidate = spaceCandidate.current;
      spaceCandidate.current = null;
      if (!candidate) return;
      event.preventDefault();
      if (!candidate.cancelled) openViewer(candidate.source, candidate.mediaId, candidate.mediaIds, candidate.focus);
    };
    const pointerDown = () => { if (spaceCandidate.current) spaceCandidate.current.cancelled = true; };
    const blur = () => { spaceCandidate.current = null; };
    window.addEventListener("keydown", keyDown, true);
    window.addEventListener("keyup", keyUp, true);
    window.addEventListener("pointerdown", pointerDown, true);
    window.addEventListener("blur", blur);
    return () => {
      window.removeEventListener("keydown", keyDown, true);
      window.removeEventListener("keyup", keyUp, true);
      window.removeEventListener("pointerdown", pointerDown, true);
      window.removeEventListener("blur", blur);
    };
  });
  const canOpenFrameInPhotoshop = selectedPhotoFrame !== null && photoshop.available && !photoshop.opening && !commandsBlocked;
  const openFrameInPhotoshop = () => {
    if (canOpenFrameInPhotoshop && selectedPhotoFrame) void photoshop.open({ kind: "frames", frameIds: [selectedPhotoFrame.id] });
  };
  const workspaceInteractionBlocked =
    mediaRemoval.active || sessionBarrierActive || graphicsFailure !== null || viewerActive !== null;
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
  const structuralCommandsBlocked =
    sheetReorderDisabled || controller.structuralMutationPending;
  const updateSheetReorder = useCallback((session: SheetReorderSession) => {
    sheetReorderSessionRef.current = session;
    setSheetReorderSession(session);
  }, []);
  useEffect(() => {
    if (!structuralCommandsBlocked) return;
    setSheetContextMenu(null);
    if (
      sheetReorderSessionRef.current.status === "preview" ||
      sheetReorderSessionRef.current.status === "invalid"
    ) {
      updateSheetReorder(
        createSheetReorderSession(projection.state.album.sheets),
      );
    }
  }, [
    projection.state.album.sheets,
    structuralCommandsBlocked,
    updateSheetReorder,
  ]);
  const previewSheetReorder = useCallback(
    (
      origin: SheetReorderSurface,
      draggedSheetId: string,
      targetIndex: number,
    ) => {
      if (structuralCommandsBlocked) return;
      const transition = reduceSheetReorderSession(
        sheetReorderSessionRef.current,
        projection.state.album.sheets,
        { type: "preview", origin, draggedSheetId, targetIndex },
      );
      updateSheetReorder(transition.session);
    },
    [
      projection.state.album.sheets,
      structuralCommandsBlocked,
      updateSheetReorder,
    ],
  );
  const dropSheetReorder = useCallback(
    (surface: SheetReorderSurface) => {
      if (structuralCommandsBlocked) {
        updateSheetReorder(
          createSheetReorderSession(projection.state.album.sheets),
        );
        return;
      }
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
      structuralCommandsBlocked,
      updateSheetReorder,
    ],
  );
  const cancelSheetReorder = useCallback(() => {
    if (structuralCommandsBlocked) {
      updateSheetReorder(
        createSheetReorderSession(projection.state.album.sheets),
      );
      return;
    }
    const transition = reduceSheetReorderSession(
      sheetReorderSessionRef.current,
      projection.state.album.sheets,
      { type: "escape" },
    );
    updateSheetReorder(transition.session);
  }, [
    projection.state.album.sheets,
    structuralCommandsBlocked,
    updateSheetReorder,
  ]);
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
  const sheetReorderGestureActive =
    sheetReorderSession.status === "preview" ||
    sheetReorderSession.status === "invalid";
  const sheetNavigationActive =
    canvasMode.kind === "normal" &&
    controller.selectedFrames.length === 0 &&
    mediaDrag === null &&
    sheetContextMenu === null &&
    !commandsBlocked &&
    !structuralCommandsBlocked &&
    !sheetReorderGestureActive;
  const openSheetContextMenu = useCallback(
    (sheetId: string, position: { x: number; y: number }) => {
      if (structuralCommandsBlocked) return;
      setSheetContextMenu({ position, sheetId });
    },
    [structuralCommandsBlocked],
  );
  useEffect(() => {
    if (commandsBlocked || (frameContextMenu?.kind !== "photo" && !controller.canAddFrame) ||
        (frameContextMenu?.kind === "frames" && !controller.canArrangeFrames)) setFrameContextMenu(null);
  }, [controller.canAddFrame, controller.canArrangeFrames, frameContextMenu?.kind, commandsBlocked, projectId]);
  const openFrameContextMenu = (frameId: string, position: { x: number; y: number }) => {
    if (commandsBlocked) return;
    if (canvasMode.kind === "normal") {
      const frame = projection.state.album.sheets.flatMap((sheet) => sheet.frames).find((frame) => frame.id === frameId);
      if (!frame?.photo) return;
      controller.canvasProps.onSelectFrame(frameId);
      const sheetId = projection.state.album.sheets.find((sheet) => sheet.frames.some((item) => item.id === frameId))?.id;
      setFrameContextMenu({ kind: "photo", position, mediaId: frame.photo.mediaId, sheetId });
      return;
    }
    if (!projection.state.album.sheets.find((sheet) => sheet.id === canvasMode.sheetId)?.frames.some((frame) => frame.id === frameId)) return;
    if (!controller.canvasProps.selectedFrameIds.includes(frameId)) controller.canvasProps.onSelectFrame(frameId);
    if (!controller.canAddFrame || !controller.canArrangeFrames) {
      const frame = projection.state.album.sheets.flatMap((sheet) => sheet.frames).find((frame) => frame.id === frameId);
      if (frame?.photo) setFrameContextMenu({ kind: "photo", position, mediaId: frame.photo.mediaId, sheetId: canvasMode.sheetId });
    } else {
      const frame = projection.state.album.sheets.flatMap((sheet) => sheet.frames).find((frame) => frame.id === frameId);
      setFrameContextMenu({ kind: "frames", position, mediaId: frame?.photo?.mediaId, sheetId: canvasMode.sheetId });
    }
  };
  const openEmptyCanvasContextMenu = (sheetId: string, position: { x: number; y: number }) => {
    if (!controller.canAddFrame || commandsBlocked || canvasMode.kind !== "sheet-editing" || canvasMode.sheetId !== sheetId) return;
    setFrameContextMenu({ kind: "empty", position });
  };
  const launchProject = useProjectLauncher(projectLauncher, commandsBlocked || mediaDrag !== null, reportCloseError);
  const [fitSheetRequest, setFitSheetRequest] = useState(0);
  const canvasNavigationBlocked = commandsBlocked || mediaDrag !== null || sheetContextMenu !== null || frameContextMenu !== null;
  useProjectCommandShortcuts({
    ...launchProject,
    selectAllFrames: controller.selectAllFrames,
    frameSelectionActive: sheetEditing && mediaDrag === null && sheetContextMenu === null && frameContextMenu === null,
    openPhotoInPhotoshop: openFrameInPhotoshop,
    photoCommandActive: selectedPhotoFrame !== null && mediaDrag === null && sheetContextMenu === null && frameContextMenu === null,
    copyFrames: () => { void controller.copyFrames(); },
    pasteFrames: () => { void controller.pasteFrames(); },
    frameClipboardActive: mediaDrag === null && sheetContextMenu === null && frameContextMenu === null,
    deleteFrames: () => { void controller.deleteFrames(); },
    arrangeFrames: (action) => { void controller.arrangeFrames(action); },
    frameCommandsActive: controller.canDeleteFrames && mediaDrag === null && sheetContextMenu === null && frameContextMenu === null,
    canDeleteSheet: implicitSheetAvailability.canDelete,
    canRedo: projection.state.canRedo,
    canUndo: projection.state.canUndo,
    closeProject: projectClose.requestClose,
    deleteSheet: () => {
      void controller.deleteSheet();
    },
    disabled: commandsBlocked || mediaDrag !== null,
    navigateToNextSheet: () => controller.navigateToAdjacentSheet("next"),
    navigateToPreviousSheet: () =>
      controller.navigateToAdjacentSheet("previous"),
    redo: controller.redo,
    save: controller.save,
    saveAs: controller.saveAs,
    sheetShortcutActive: controller.selectedFrames.length === 0,
    sheetCommandsDisabled: structuralCommandsBlocked,
    sheetNavigationActive,
    undo: controller.undo,
  });
  const applicationMenus = createProjectApplicationMenus({
    fitSheet: sheetEditing && !canvasNavigationBlocked ? () => setFitSheetRequest((request) => request + 1) : undefined,
    ...launchProject,
    selectAllFrames: controller.selectAllFrames,
    canSelectAllFrames: controller.canSelectAllFrames,
    generateProjects: generationLauncher && !structuralCommandsBlocked ? () => { void openProjectGeneration(generationLauncher, runProjectMutation, changeSessionBarrier).catch(error => reportCloseError(String(error))); } : undefined,
    openSettings: photoshopPort ? () => void photoshop.openSettings("performance") : undefined,
    saveLayout: controller.saveLayout,
    canSaveLayout: controller.canSaveLayout,
    copyFrames: () => { void controller.copyFrames(); },
    pasteFrames: () => { void controller.pasteFrames(); },
    canCopyFrames: controller.canCopyFrames,
    canPasteFrames: controller.canPasteFrames,
    swapFrameContents: () => { void controller.swapFrameContents(); },
    canSwapFrameContents: controller.canSwapFrameContents,
    addFrame: () => { void controller.addFrame(); },
    canAddFrame: controller.canAddFrame,
    arrangeFrames: (action) => { void controller.arrangeFrames(action); },
    canArrangeFrames: controller.canArrangeFrames,
    addSheetAfter: () => {
      void controller.addSheetAfter();
    },
    addSheetBefore: () => {
      void controller.addSheetBefore();
    },
    canAddAfter: implicitSheetAvailability.canAddAfter,
    canDuplicate: implicitSheetAvailability.canDuplicate,
    duplicateSheet: () => { void controller.duplicateSheet(); },
    canAddBefore: implicitSheetAvailability.canAddBefore,
    canConvertEdge: implicitSheetAvailability.canConvertEdge,
    canDelete: implicitSheetAvailability.canDelete,
    canExport: controller.canvasProps.centeredSheetId !== null,
    canRedo: projection.state.canRedo,
    canUndo: projection.state.canUndo,
    contextualPanelVisible: workspacePanels.panels.inspector.visible,
    closeProject: () => void projectClose.requestClose(),
    convertEdge: () => {
      void controller.convertEdge();
    },
    deleteSheet: () => {
      void controller.deleteSheet();
    },
    exportSheet: () => exportControlRef.current?.start(),
    exportAlbum: () => exportControlRef.current?.start("album"),
    mediaPanelVisible: workspacePanels.panels.media.visible,
    redo: () => void controller.redo(),
    save: () => void controller.save(),
    saveAs: () => void controller.saveAs(),
    structuralCommandsDisabled: structuralCommandsBlocked,
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

  const noticeInInspector = inspectorContext.kind === "sheet" &&
    workspacePanels.panels.inspector.visible &&
    (workspacePreferences.preferences.inspectorSections["sheet.design"] ?? true);
  const noticeSheetId = editingSheet?.sheetId ?? null;
  const dismissLayoutNotice = controller.layoutCatalog.dismissNotice;
  useLayoutEffect(() => {
    dismissLayoutNotice();
  }, [noticeInInspector, noticeSheetId, dismissLayoutNotice]);
  const layoutNotice = sheetEditing && controller.layoutCatalog.notice ? <LayoutCatalogNotice
    message={controller.layoutCatalog.notice}
    onDismiss={controller.layoutCatalog.dismissNotice}
  /> : null;

  return (
    <div className="app-shell ui-chrome-selection-scope">
      <ApplicationHeader
        context={projection.state.projectName}
        metadata={projectMetadata}
        status={projection.state.dirty ? "alterações não salvas" : "salvo"}
      />

      <div className="commandbar">
        <div className="layout-catalog-menu-feedback">
          <ApplicationMenuBar
            disabled={commandsBlocked || mediaDrag !== null}
            groups={applicationMenus}
          />
          {!noticeInInspector && layoutNotice}
        </div>
        <ExportPreviewControl
          ref={exportControlRef}
          dialogPort={projectDialogPort}
          disabled={
            controller.importPending ||
            projectClose.interactionBlocked ||
            sessionBarrierActive ||
            graphicsFailure !== null ||
            viewerActive !== null
          }
          exportMediaPort={exportMediaPort}
          onProjectionChange={onProjectionChange}
          exportPipelinePort={exportPipelinePort}
          onActiveChange={setExportActive}
          projectId={projection.state.projectId}
          selection={exportSelection}
          sheets={projection.composition.sheets.map(sheet => ({ sheetId: sheet.sheetId, number: sheet.number, pageCount: sheet.activeSides === "both" ? 2 : 1 }))}
        />
      </div>

      <div
        aria-busy={workspaceInteractionBlocked || undefined}
        className="workspace-grid"
        inert={workspaceInteractionBlocked}
        ref={workspacePanels.workspaceRef}
        style={workspaceStyle}
      >
        <section
          id="continuous-canvas"
          className={`canvas-section${controller.layoutPanel.visible ? " canvas-section--layouts" : ""}`}
          aria-label="Área de composição"
        >
          {controller.layoutPanel.visible && <LayoutPanel controller={controller.layoutPanel}
            catalog={controller.layoutCatalog}
            sheet={projection.composition.sheets.find((sheet) => sheet.sheetId === controller.layoutPanel.sheetId)!} />}
          <AlbumCanvas
            {...controller.canvasProps}
            editingNavigation={{ disabled: canvasNavigationBlocked, fitRequest: fitSheetRequest,
              onPanGesture: () => { if (spaceCandidate.current) spaceCandidate.current.cancelled = true; } }}
            onOpenFrameContextMenu={openFrameContextMenu}
            onOpenEmptyCanvasContextMenu={openEmptyCanvasContextMenu}
            draggedPhotoId={draggedPhotoId}
            mediaDrag={mediaDrag}
            onPhotoDragCancel={() => setMediaDrag(null)}
            sheetReorder={{
              disabled: structuralCommandsBlocked,
              representation: sheetReorderRepresentation(
                sheetReorderSession,
                "bar",
              ),
              status: sheetReorderSession.status,
              onPreview: (draggedSheetId, targetIndex) =>
                previewSheetReorder("bar", draggedSheetId, targetIndex),
              onDrop: () => dropSheetReorder("bar"),
              onCancel: cancelSheetReorder,
              onSelect: controller.canvasProps.onFocusSheet,
            }}
            mediaPreviewUrls={mediaPreviewUrls}
            missingMediaIds={missingMediaIds}
            technicalGuides={{
              bleedUm: projection.state.document.bleedUm,
              safetyUm: projection.state.document.safetyUm,
            }}
            onMediaDemandChange={setCanvasMediaDemand}
            onGraphicsUnavailable={onGraphicsUnavailable}
            onOpenSheetContextMenu={openSheetContextMenu}
          />
        </section>

        {mediaPanelVisible && (
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
          saveLayout={{ enabled: controller.canSaveLayout, onSave: controller.saveLayout,
            feedback: noticeInInspector ? layoutNotice : null }}
          key={projectId}
          frameStyle={controller.frameStyle}
          photoZoom={controller.photoZoom}
          sheetDesign={controller.sheetDesign}
          photoOrientation={{ disabled: !controller.canOrientPhotos,
            onAction: (action) => { void controller.orientPhotos(action); }, angle: controller.photoAngle }}
          photoEffects={{ disabled: !controller.canApplyPhotoEffects,
            onToggleBlackAndWhite: () => { void controller.togglePhotoBlackAndWhite(); } }}
          context={inspectorContext}
          displayedPhotoZoom={displayedPhotoZoom}
          document={projection.state.document}
          presentationUnit={presentationUnit}
          mediaItems={projection.state.album.media}
          sheetStates={projection.state.album.sheets}
          sheets={controller.canvasProps.composition.sheets}
          visualDefaults={projection.state.album.visualDefaults}
          frameGapUm={projection.state.layoutSettings.gapUm}
          focusedSheetId={controller.canvasProps.focusedSheetId}
          mediaPreviews={mediaPreviews}
          revision={projection.state.revision}
          onApplyAlbumInformation={albumInformationApply.requestApply}
          onApplyAlbumDesign={controller.applyAlbumDesign}
          onPresentationUnitChange={changePresentationUnit}
          onValidateAlbumInformation={projectCorePort.validateAlbumInformation}
          onNavigateToSheet={controller.navigateToSheet}
          onOpenSheetContextMenu={openSheetContextMenu}
          sheetReorder={{
            disabled: structuralCommandsBlocked,
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

        <MediaPanel
          onViewPhoto={imageViewerWindowPort ? (mediaId, mediaIds, trigger) => openViewer("panel", mediaId, mediaIds, trigger) : undefined}
          photoshopAvailable={photoshop.available && !photoshop.opening && !commandsBlocked}
          onOpenInPhotoshop={(mediaId) => { if (!commandsBlocked) void photoshop.open({ kind: "panel", mediaIds: [mediaId] }); }}
          dropPort={mediaDropPort}
          key={`media-${projectId}`}
          mediaFiles={mediaFiles}
          hidden={!workspacePanels.panels.media.visible || controller.layoutPanel.visible}
          ref={mediaPanelRef}
          mediaFolders={projection.state.album.mediaFolders}
          onEditMediaFolder={controller.editMediaFolder}
          onValidateMediaFolderName={projectCorePort.validateMediaFolderName}
          folderValidationKey={`${projection.state.projectId}:${projection.state.revision}`}
          mediaItems={projection.state.album.media}
          mediaUsage={projection.mediaUsage}
          onFillPhoto={controller.fillMedia}
          onApplyDecorative={controller.applyDecorative}
          onRemoveMedia={(ids) => { if (!commandsBlocked) void mediaRemoval.request(ids); }}
          selectionRequest={mediaSelectionRequest}
          importPending={controller.importPending}
          onImportMedia={(selection) => {
            void controller.importMedia(selection).then((mediaId) => {
              if (mediaId) setMediaSelectionRequest({ mediaId });
            });
          }}
          onMediaDragChange={setMediaDrag}
          dragThreshold={controller.frameStyle.dragThreshold}
          onRelinkMedia={controller.relinkMedia}
          onReplaceMedia={controller.replaceMedia}
          onRetryUnavailableMedia={(mediaId) => controller.retryUnavailableMedia(
            (publish) => onRetryUnavailableMedia(mediaId, publish),
          )}
          relinkDisabled={commandsBlocked}
          preferences={{
            kind: "controlled",
            activeKind: workspacePreferences.preferences.mediaPanelActiveKind,
            onActiveKindChange: (mediaKind) => workspacePreferences.update({ kind: "mediaPanelActiveKind", mediaKind }),
            onSortKeyChange: (mediaKind, sortKey) => workspacePreferences.update({ kind: "mediaPanelSortKey", mediaKind, sortKey }),
            persistent: workspacePreferences.preferences.mediaPanel,
            thumbnailSize:
              workspacePreferences.preferences.mediaThumbnailSize,
            onThumbnailSizeChange: (size) =>
              workspacePreferences.update({
                kind: "mediaThumbnailSize",
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
        />
      </div>

      {photoshop.error && <div className="photoshop-operation-notice"><InlineNotice role="alert" tone="error">
        <p>{photoshop.error.message}</p>
        {photoshop.error.configure && <ActionButton onClick={() => void photoshop.openSettings()}>Configurar Photoshop…</ActionButton>}
        <ActionButton onClick={photoshop.dismissError}>Fechar aviso</ActionButton>
      </InlineNotice></div>}
      {frameContextMenu?.kind === "frames" || frameContextMenu?.kind === "photo" ? <FrameContextMenu position={frameContextMenu.position}
        editing={frameContextMenu.kind === "frames"}
        hasPhoto={controller.selectedFrames.some((frame) => frame.photo !== null)}
        onViewPhoto={imageViewerWindowPort && frameContextMenu.mediaId && frameContextMenu.sheetId ? () => {
          const ids = sheetViewerMediaIds(projection.composition.sheets.find((sheet) => sheet.sheetId === frameContextMenu.sheetId) ?? null);
          openViewer("sheet", frameContextMenu.mediaId!, ids, document.querySelector<HTMLElement>(".canvas-host canvas"));
        } : undefined}
        canOpenInPhotoshop={canOpenFrameInPhotoshop}
        onOpenInPhotoshop={openFrameInPhotoshop}
        onSwapContents={() => { void controller.swapFrameContents(); }}
        canSwapContents={controller.canSwapFrameContents}
        onDelete={() => { void controller.deleteFrames(); }}
        onArrange={(action) => { void controller.arrangeFrames(action); }}
        onDismiss={() => setFrameContextMenu(null)} /> : null}
      {frameContextMenu?.kind === "empty" ? (
        <ContextMenuSurface label="Área vazia da área de edição" position={frameContextMenu.position}
          onDismiss={() => setFrameContextMenu(null)}>
          <MenuItem label={projectCommandDescriptor("add-frame").label} onClick={() => {
            void controller.addFrame();
            setFrameContextMenu(null);
          }} />
        </ContextMenuSurface>
      ) : null}
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
          onDuplicate={() => {
            void controller.duplicateSheet(sheetContextMenu.sheetId);
          }}
          onAddBefore={() => {
            void controller.addSheetBefore(sheetContextMenu.sheetId);
          }}
          onConvertEdge={() => {
            void controller.convertEdge(sheetContextMenu.sheetId);
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
  const sheetLabel = album.sheets.length === 1 ? "lâmina" : "lâminas";
  return `${width}×${height} ${displayUnitLabel(presentationUnit)} · ${album.sheets.length} ${sheetLabel}`;
}
