import { useCallback, useEffect, useMemo, useState } from "react";

import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
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
}

export function useProjectEditorController({
  interactionBlocked = false,
  projection,
  runProjectMutation,
  projectCorePort,
  onProjectionChange,
  onSaveAsBarrierChange,
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
  const structuralCommandsDisabled =
    interactionBlocked || canvasMode.kind === "sheet-editing";
  const [pendingAffectedSheetId, setPendingAffectedSheetId] = useState<
    string | null
  >(null);

  useEffect(() => {
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
    onProjectionChange,
    onAffectedFrame: navigation.selectFrame,
    onAffectedSheet: setPendingAffectedSheetId,
    onSaveAsBarrierChange,
  });
  const selectedFrame = useMemo(
    () =>
      projection.state.album.sheets
        .flatMap((sheet) => sheet.frames)
        .find((frame) => frame.id === navigation.selectedFrameId) ??
      null,
    [projection.state.album.sheets, navigation.selectedFrameId],
  );
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
    selectedFrameId: navigation.selectedFrameId,
    focusedSheetId: navigation.focusedSheetId,
    centeredSheetId: navigation.centeredSheetId,
    viewport: navigation.viewport,
    photoZoomPreview: photoGestures.photoZoomPreview,
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

  const addSheetBefore = (sheetId = navigation.implicitSheetId) => {
    if (structuralCommandsDisabled || !sheetId) return Promise.resolve(false);
    return mutations.applyWithOutcome({
      kind: "addSheet",
      anchorSheetId: sheetId,
      position: "before",
    });
  };

  const addSheetAfter = (sheetId = navigation.implicitSheetId) => {
    if (structuralCommandsDisabled || !sheetId) return Promise.resolve(false);
    return mutations.applyWithOutcome({
      kind: "addSheet",
      anchorSheetId: sheetId,
      position: "after",
    });
  };

  const deleteSheet = (sheetId = navigation.implicitSheetId) => {
    if (structuralCommandsDisabled || !sheetId) return Promise.resolve(false);
    return mutations.applyWithOutcome({ kind: "deleteSheet", sheetId });
  };

  const reorderSheet = (sheetId: string, targetIndex: number) => {
    if (structuralCommandsDisabled) return Promise.resolve(false);
    return mutations.applyWithOutcome({
      kind: "reorderSheet",
      sheetId,
      targetIndex,
    });
  };

  return {
    message: mutations.message,
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom: photoGestures.displayedPhotoZoom,
    displayedPhotoPanX: photoGestures.displayedPhotoPanX,
    zoomCommitting: photoGestures.zoomCommitting,
    sheetCount: projection.state.album.sheets.length,
    structuralCommandsDisabled,
    canvasProps,
    navigateToSheet: navigation.navigateToSheet,
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
