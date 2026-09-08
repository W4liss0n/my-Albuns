import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type { PointerDragThreshold, ProjectCorePort } from "../application/projectPorts";
import type { PrepareImportedMedia } from "../application/mediaPreviews";
import type { SheetStructureIntent } from "../application/sheetStructure";
import type { EditorProjection, FrameStackAction, PhotoOrientationAction } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { CANVAS_MICROMETERS_PER_PIXEL } from "./canvasGeometry";
import type {
  AlbumCanvasMode,
  AlbumCanvasProps,
} from "./albumCanvasContract";
import { useCanvasModeKeyboardShortcuts } from "./useCanvasModeKeyboardShortcuts";
import { usePhotoGestures } from "./usePhotoGestures";
import { useProjectMutations } from "./useProjectMutations";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import { useProjectNavigation } from "./useProjectNavigation";

interface ProjectEditorControllerInput {
  interactionBlocked?: boolean;
  projection: EditorProjection;
  runProjectMutation: ProjectMutationRunner;
  projectCorePort: ProjectCorePort;
  onProjectionChange(projection: EditorProjection): void;
  onSaveAsBarrierChange?(active: boolean): void;
  prepareImportedMedia?: PrepareImportedMedia;
}

export function useProjectEditorController({
  interactionBlocked = false,
  projection,
  runProjectMutation,
  projectCorePort,
  onProjectionChange,
  onSaveAsBarrierChange,
  prepareImportedMedia,
}: ProjectEditorControllerInput) {
  const navigation = useProjectNavigation(projection);
  const canvasMode = useMemo<AlbumCanvasMode>(
    () =>
      navigation.editingSheetId
        ? {
            kind: "sheet-editing",
            sheetId: navigation.editingSheetId,
          }
        : { kind: "normal" },
    [navigation.editingSheetId],
  );
  const structuralMutationAttemptRef = useRef(0);
  const structuralMutationPendingRef = useRef(false);
  const [structuralMutationPending, setStructuralMutationPending] =
    useState(false);
  const structuralCommandsDisabled =
    interactionBlocked || canvasMode.kind === "sheet-editing";
  const [pendingAffectedSheetId, setPendingAffectedSheetId] = useState<
    string | null
  >(null);

  useEffect(() => {
    structuralMutationAttemptRef.current += 1;
    structuralMutationPendingRef.current = false;
    setStructuralMutationPending(false);
    setPendingAffectedSheetId(null);
  }, [projection.state.projectId]);

  useEffect(() => {
    if (
      !pendingAffectedSheetId ||
      !projection.composition.sheets.some(
        (sheet) => sheet.sheetId === pendingAffectedSheetId,
      )
    ) {
      return;
    }
    navigation.navigateToSheet(pendingAffectedSheetId);
    setPendingAffectedSheetId(null);
  }, [
    navigation.navigateToSheet,
    pendingAffectedSheetId,
    projection.composition.sheets,
  ]);

  const mutations = useProjectMutations({
    projection,
    runProjectMutation,
    onProjectionChange: (next) => {
      navigation.synchronizeProjection(next);
      onProjectionChange(next);
    },
    onAffectedFrame: navigation.selectFrame,
    onAffectedSheet: setPendingAffectedSheetId,
    onSaveAsBarrierChange,
    prepareImportedMedia,
  });
  const [dragThreshold, setDragThreshold] = useState<PointerDragThreshold | null>(null);
  const reportInteractionError = mutations.reportInteractionError;
  useEffect(() => {
    let request = 0;
    let active = true;
    setDragThreshold(null);
    const readThreshold = () => {
      const currentRequest = ++request;
      setDragThreshold(null);
      void projectCorePort.readFrameDragThreshold().then((threshold) => {
        if (active && currentRequest === request) setDragThreshold(threshold);
      }).catch((error: unknown) => {
        if (active && currentRequest === request) {
          reportInteractionError(error instanceof Error ? error.message : String(error));
        }
      });
    };
    readThreshold();
    window.addEventListener("resize", readThreshold);
    return () => {
      active = false;
      window.removeEventListener("resize", readThreshold);
    };
  }, [navigation.editingSheetId, projection.state.projectId, projectCorePort, reportInteractionError]);
  const selectedFrames = useMemo(
    () =>
      projection.state.album.sheets
        .flatMap((sheet) => sheet.frames)
        .filter((frame) => navigation.selectedFrameIds.includes(frame.id)),
    [projection.state.album.sheets, navigation.selectedFrameIds],
  );
  const selectedFrame = selectedFrames.length === 1 ? selectedFrames[0] : null;
  const canAddFrame = canvasMode.kind === "sheet-editing" && !interactionBlocked;
  const addFrame = () => {
    if (!canAddFrame || canvasMode.kind !== "sheet-editing") return Promise.resolve(false);
    return mutations.applyWithOutcome({ kind: "addFrame", sheetId: canvasMode.sheetId });
  };
  const canArrangeFrames = canvasMode.kind === "sheet-editing" && selectedFrames.length > 0 && !interactionBlocked;
  const canOrientPhotos = selectedFrames.some((frame) => frame.photo !== null) && !interactionBlocked;
  const orientPhotos = (action: PhotoOrientationAction) => {
    if (!canOrientPhotos) return Promise.resolve(false);
    return mutations.orientPhotos([...navigation.selectedFrameIds], action);
  };
  const canDeleteFrames = canArrangeFrames;
  const canCopyFrames = canArrangeFrames;
  const canPasteFrames = canAddFrame && (projection.canPasteFrames || mutations.frameCopyPending);
  const copyFrames = () => {
    if (!canCopyFrames) return Promise.resolve(false);
    return mutations.copyFrames([...navigation.selectedFrameIds]);
  };
  const pasteFrames = () => {
    // Queue a rapid Ctrl+V after Ctrl+C even before its projection is rendered.
    // Clipboard availability is checked again against the authoritative queued result.
    if (!canAddFrame || canvasMode.kind !== "sheet-editing") return Promise.resolve(false);
    const sheetId = canvasMode.sheetId;
    const desiredOffsetUm = navigation.canvasScale ? Math.round(16 * CANVAS_MICROMETERS_PER_PIXEL / navigation.canvasScale) : 0;
    return mutations.pasteFrames(sheetId, desiredOffsetUm, (ids, next) => {
      const view = useEditorView.getState();
      if (view.projectId !== next.state.projectId || view.editingSheetId !== sheetId) return;
      view.selectFrames(ids);
    });
  };
  const canSwapFrameContents = canvasMode.kind === "sheet-editing" &&
    selectedFrames.length === 2 && selectedFrames.some((frame) => frame.photo !== null) &&
    !interactionBlocked;
  const swapFrameContents = () => {
    if (!canSwapFrameContents) return Promise.resolve(false);
    return mutations.applyIntent({ kind: "swapFrameContents", frameIds: [...navigation.selectedFrameIds] });
  };
  const deleteFrames = () => {
    if (!canDeleteFrames) return Promise.resolve(false);
    return mutations.applyIntent({ kind: "deleteFrames", frameIds: [...navigation.selectedFrameIds] });
  };
  const arrangeFrames = (action: FrameStackAction) => {
    if (!canArrangeFrames) return Promise.resolve(false);
    return mutations.applyIntent({ kind: "arrangeFrames", frameIds: [...navigation.selectedFrameIds], action });
  };
  const selectedComposedPhoto = useMemo(
    () =>
      projection.composition.sheets
        .flatMap((sheet) => sheet.frames)
        .find(
          (frame) => frame.frameId === navigation.selectedFrameId,
        )?.photo ?? null,
    [projection.composition.sheets, navigation.selectedFrameId],
  );
  const photoGestures = usePhotoGestures({
    projection,
    selectedFrame,
    selectedFrameId: navigation.selectedFrameId,
    commitInteraction: mutations.commitInteraction,
  });

  const exitSheetEditing = useCallback(() => {
    const editedSheetId = navigation.editingSheetId;
    navigation.exitSheetEdit();
    if (editedSheetId) {
      navigation.focusSheet(editedSheetId);
      navigation.centerSheet(editedSheetId);
    }
  }, [
    navigation.centerSheet,
    navigation.editingSheetId,
    navigation.exitSheetEdit,
    navigation.focusSheet,
  ]);

  const enterSheetEditing = useCallback(
    (sheetId: string) => {
      if (
        interactionBlocked ||
        navigation.editingSheetId !== null ||
        !projection.state.album.sheets.some(
          (sheet) => sheet.id === sheetId,
        )
      ) {
        return;
      }
      navigation.enterSheetEdit(sheetId);
    },
    [
      interactionBlocked,
      navigation.editingSheetId,
      navigation.enterSheetEdit,
      projection.state.album.sheets,
    ],
  );

  useCanvasModeKeyboardShortcuts({
    implicitSheetId: navigation.implicitSheetId,
    interactionBlocked,
    mode: canvasMode,
    onEnterSheetEditing: enterSheetEditing,
    onExitSheetEditing: exitSheetEditing,
  });

  const swapSheetSides = (sheetId: string) => {
    if (structuralCommandsDisabled || structuralMutationPendingRef.current ||
        projection.state.album.sheets.find((sheet) => sheet.id === sheetId)?.activeSides !== "both") {
      return Promise.resolve(false);
    }
    return mutations.swapSheetSides(sheetId);
  };

  const canvasProps: AlbumCanvasProps = {
    projectId: projection.state.projectId,
    mode: canvasMode,
    composition: projection.composition,
    sheetBarMetadata: projection.state.album.sheets.map((sheet) => ({
      sheetId: sheet.id,
      pageNumbers: sheet.pageNumbers,
      // UI placeholder: the current projection does not expose per-Sheet
      // Layout locking to the renderer yet.
      layoutLocked: false,
    })),
    continuousCanvasLayout: navigation.canvasLayout,
    selectedFrameIds: navigation.selectedFrameIds,
    focusedSheetId: navigation.focusedSheetId,
    centeredSheetId: navigation.centeredSheetId,
    viewport: navigation.viewport,
    photoZoomPreview: photoGestures.photoZoomPreview,
    sheetSideSwap: {
      disabled: structuralCommandsDisabled || structuralMutationPending,
      onSwap: (sheetId) => { void swapSheetSides(sheetId); },
    },
    frameGeometry: {
      disabled: interactionBlocked,
      dragThreshold,
      preview: (edit) => projectCorePort.previewFrameGeometry(edit),
      commit: mutations.commitFrameGeometry,
      onError: reportInteractionError,
    },
    frameContentSwap: {
      disabled: interactionBlocked || structuralMutationPending || canvasMode.kind !== "normal",
      dragThreshold,
      resolveTarget: (point) => projectCorePort.resolvePhotoDropTarget(point.sheetId, point.xUm, point.yUm),
      commit: (sourceFrameId, point) => interactionBlocked || structuralMutationPending || canvasMode.kind !== "normal"
        ? Promise.resolve(false) : mutations.swapFrameContentsAtPoint(sourceFrameId, point),
      onError: reportInteractionError,
    },
    onSelectFrame: navigation.selectFrame,
    onEditSheet: enterSheetEditing,
    onFocusSheet: navigation.focusSheet,
    onCenteredSheetChange: navigation.centerSheet,
    onViewportChange: navigation.setViewport,
    onTransformPreview: photoGestures.onTransformPreview,
    onTransformCommit: photoGestures.onTransformCommit,
    onResolvePhotoDropTarget: async (_mediaId, point) =>
      projectCorePort.resolvePhotoDropTarget(
        point.sheetId,
        point.xUm,
        point.yUm,
      ),
    onDropPhoto: (mediaId, point) =>
      mutations.dropPhoto({
        kind: "dropPhoto",
        sheetId: point.sheetId,
        mediaId,
        xUm: point.xUm,
        yUm: point.yUm,
        mode: canvasMode.kind === "sheet-editing" ? "edit" : "normal",
      }),
    onCanvasMetricsChange: navigation.handleCanvasMetricsChange,
  };

  async function applyStructuralIntent(intent: SheetStructureIntent) {
    if (structuralCommandsDisabled || structuralMutationPendingRef.current) {
      return false;
    }
    const attempt = structuralMutationAttemptRef.current + 1;
    structuralMutationAttemptRef.current = attempt;
    structuralMutationPendingRef.current = true;
    setStructuralMutationPending(true);
    try {
      return await mutations.applyWithOutcome(intent);
    } finally {
      if (structuralMutationAttemptRef.current === attempt) {
        structuralMutationPendingRef.current = false;
        setStructuralMutationPending(false);
      }
    }
  }

  const addSheetBefore = (sheetId = navigation.implicitSheetId) => {
    if (!sheetId) return Promise.resolve(false);
    return applyStructuralIntent({
      kind: "addSheet",
      anchorSheetId: sheetId,
      position: "before",
    });
  };

  const addSheetAfter = (sheetId = navigation.implicitSheetId) => {
    if (!sheetId) return Promise.resolve(false);
    return applyStructuralIntent({
      kind: "addSheet",
      anchorSheetId: sheetId,
      position: "after",
    });
  };

  const deleteSheet = (sheetId = navigation.implicitSheetId) => {
    if (!sheetId) return Promise.resolve(false);
    return applyStructuralIntent({ kind: "deleteSheet", sheetId });
  };

  const convertEdge = (sheetId = navigation.implicitSheetId) => {
    if (!sheetId) return Promise.resolve(false);
    return applyStructuralIntent({ kind: "convertEdgeSheet", sheetId });
  };

  const reorderSheet = (sheetId: string, targetIndex: number) => {
    return applyStructuralIntent({
      kind: "reorderSheet",
      sheetId,
      targetIndex,
    });
  };

  return {
    canOrientPhotos,
    orientPhotos,
    addFrame,
    canAddFrame,
    canDeleteFrames,
    canCopyFrames,
    canPasteFrames,
    copyFrames,
    pasteFrames,
    deleteFrames,
    canSwapFrameContents,
    swapFrameContents,
    swapSheetSides,
    arrangeFrames,
    canArrangeFrames,
    message: mutations.message,
    importPending: mutations.importPending,
    imageProcessingProgress: mutations.imageProcessingProgress,
    imageProcessingProblems: mutations.imageProcessingProblems,
    dismissImageProcessingProblems: mutations.dismissImageProcessingProblems,
    retryUnavailableMedia: mutations.retryUnavailableMedia,
    photoImportResult: mutations.photoImportResult,
    dismissPhotoImportResult: mutations.dismissPhotoImportResult,
    selectedFrame,
    selectedComposedPhoto,
    selectedFrames,
    displayedPhotoZoom: photoGestures.displayedPhotoZoom,
    displayedPhotoPanX: photoGestures.displayedPhotoPanX,
    zoomCommitting: photoGestures.zoomCommitting,
    sheetCount: projection.state.album.sheets.length,
    structuralCommandsDisabled,
    structuralMutationPending,
    canvasProps,
    navigateToSheet: navigation.navigateToSheet,
    navigateToAdjacentSheet: navigation.navigateToAdjacentSheet,
    beginZoomGesture: photoGestures.beginZoomGesture,
    updateZoomGesture: photoGestures.updateZoomGesture,
    finishZoomGesture: photoGestures.finishZoomGesture,
    applyAlbumInformation: mutations.applyAlbumInformation,
    applyAlbumDesign: mutations.applyAlbumDesign,
    applyDpi: mutations.applyDpi,
    relinkMedia: mutations.relinkMedia,
    importPhoto: mutations.importPhoto,
    addSheetBefore,
    addSheetAfter,
    convertEdge,
    deleteSheet,
    reorderSheet,
    save: mutations.save,
    saveAs: mutations.saveAs,
    undo: mutations.undo,
    redo: mutations.redo,
    fillMedia: (mediaId: string) => {
      if (navigation.implicitSheetId) {
        void mutations.applyPhotoWithStatus({
          kind: "addPhoto",
          sheetId: navigation.implicitSheetId,
          mediaId,
          mode: canvasMode.kind === "sheet-editing" ? "edit" : "normal",
        });
      }
    },
    dismissFeedback: mutations.dismissFeedback,
  };
}
