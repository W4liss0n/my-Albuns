import { useMemo } from "react";

import type { EditorProjection } from "../domain/project";
import type { AlbumCanvasProps } from "./albumCanvasContract";
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

  const canvasProps: AlbumCanvasProps = {
    projectId: projection.state.projectId,
    composition: projection.composition,
    continuousCanvasLayout: navigation.canvasLayout,
    selectedFrameId: navigation.selectedFrameId,
    focusedSheetId: navigation.focusedSheetId,
    centeredSheetId: navigation.centeredSheetId,
    viewport: navigation.viewport,
    photoZoomPreview: photoGestures.photoZoomPreview,
    onSelectFrame: navigation.selectFrame,
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
    relinkMedia: mutations.relinkMedia,
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
