import { useCallback, useMemo } from "react";

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
}

export function useProjectEditorController({
  interactionBlocked = false,
  projection,
  runProjectMutation,
  projectCorePort,
  onProjectionChange,
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
  const mutations = useProjectMutations({
    projection,
    runProjectMutation,
    onProjectionChange,
    onAffectedFrame: navigation.selectFrame,
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

  return {
    message: mutations.message,
    selectedFrame,
    selectedComposedPhoto,
    displayedPhotoZoom: photoGestures.displayedPhotoZoom,
    displayedPhotoPanX: photoGestures.displayedPhotoPanX,
    zoomCommitting: photoGestures.zoomCommitting,
    sheetCount: projection.state.album.sheets.length,
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
