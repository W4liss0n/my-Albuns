import { useCallback, useEffect, useMemo, useState } from "react";

import type { EditorProjection } from "../domain/project";
import type {
  AlbumCanvasMode,
  AlbumCanvasProps,
} from "./albumCanvasContract";
import {
  useCanvasModeKeyboardShortcuts,
} from "./useCanvasModeKeyboardShortcuts";
import { usePhotoGestures } from "./usePhotoGestures";
import { useProjectMutations } from "./useProjectMutations";
import type { ProjectMutationRunner } from "./useProjectMutationRunner";
import { useProjectNavigation } from "./useProjectNavigation";

interface ProjectEditorControllerInput {
  interactionBlocked?: boolean;
  projection: EditorProjection;
  runProjectMutation: ProjectMutationRunner;
  onProjectionChange(projection: EditorProjection): void;
}

export function useProjectEditorController({
  interactionBlocked = false,
  projection,
  runProjectMutation,
  onProjectionChange,
}: ProjectEditorControllerInput) {
  const navigation = useProjectNavigation(projection);
  const [canvasMode, setCanvasMode] = useState<AlbumCanvasMode>({
    kind: "normal",
  });
  const mutations = useProjectMutations({
    interactionBlocked,
    projection,
    runProjectMutation,
    onProjectionChange,
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
    const editedSheetId =
      canvasMode.kind === "sheet-editing" ? canvasMode.sheetId : null;
    navigation.selectFrame(null);
    if (editedSheetId) {
      navigation.focusSheet(editedSheetId);
      navigation.centerSheet(editedSheetId);
    }
    setCanvasMode({ kind: "normal" });
  }, [
    canvasMode,
    navigation.centerSheet,
    navigation.focusSheet,
    navigation.selectFrame,
  ]);

  const enterSheetEditing = useCallback(
    (sheetId: string) => {
      if (
        interactionBlocked ||
        canvasMode.kind !== "normal" ||
        !projection.state.album.sheets.some(
          (sheet) => sheet.id === sheetId,
        )
      ) {
        return;
      }
      const selectedFrameBelongsToTarget =
        navigation.selectedFrameId === null ||
        projection.state.album.sheets.some(
          (sheet) =>
            sheet.id === sheetId &&
            sheet.frames.some(
              (frame) => frame.id === navigation.selectedFrameId,
            ),
        );
      if (!selectedFrameBelongsToTarget) {
        navigation.selectFrame(null);
      }
      navigation.focusSheet(sheetId);
      setCanvasMode({ kind: "sheet-editing", sheetId });
    },
    [
      canvasMode.kind,
      interactionBlocked,
      navigation.focusSheet,
      navigation.selectFrame,
      navigation.selectedFrameId,
      projection.state.album.sheets,
    ],
  );

  useEffect(() => {
    setCanvasMode({ kind: "normal" });
  }, [projection.state.projectId]);

  useEffect(() => {
    const editingSheetStillExists =
      canvasMode.kind === "normal" ||
      projection.state.album.sheets.some(
        (sheet) => sheet.id === canvasMode.sheetId,
      );
    if (editingSheetStillExists) return;
    exitSheetEditing();
  }, [
    canvasMode,
    exitSheetEditing,
    projection.state.album.sheets,
  ]);

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
    onCanvasMetricsChange: navigation.handleCanvasMetricsChange,
  };

  return {
    busy: mutations.busy,
    message: mutations.message,
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom: photoGestures.displayedPhotoZoom,
    displayedPhotoPanX: photoGestures.displayedPhotoPanX,
    zoomCommitting: photoGestures.zoomCommitting,
    sheetCount: projection.state.album.sheets.length,
    photoCount: projection.state.album.sheets.reduce(
      (count, sheet) =>
        count + sheet.frames.filter((frame) => frame.photo).length,
      0,
    ),
    canvasProps,
    navigateToSheet: navigation.navigateToSheet,
    beginZoomGesture: photoGestures.beginZoomGesture,
    updateZoomGesture: photoGestures.updateZoomGesture,
    finishZoomGesture: photoGestures.finishZoomGesture,
    applyDpi: mutations.applyDpi,
    save: mutations.save,
    undo: mutations.undo,
    redo: mutations.redo,
    fillMedia: (mediaId: string) => {
      if (navigation.implicitSheetId) {
        void mutations.applyWithStatus({
          kind: "fillLeftmostPlaceholder",
          sheetId: navigation.implicitSheetId,
          mediaId,
        });
      }
    },
    dismissFeedback: mutations.dismissFeedback,
  };
}
