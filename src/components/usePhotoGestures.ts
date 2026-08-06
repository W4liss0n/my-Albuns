import { useEffect, useRef, useState } from "react";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import type { PhotoTransformPreview } from "./albumCanvasContract";

type ProjectedFrame =
  EditorProjection["state"]["album"]["sheets"][number]["frames"][number];

interface ZoomDraft {
  projectId: string;
  frameId: string;
  startValue: number;
  value: number;
  committing: boolean;
}

interface ScopedPhotoTransformPreview {
  projectId: string;
  preview: PhotoTransformPreview;
}

interface PhotoGesturesInput {
  projection: EditorProjection;
  selectedFrame: ProjectedFrame | null;
  selectedFrameId: string | null;
  commitInteraction(intent: ProjectIntent): Promise<boolean>;
}

export function usePhotoGestures({
  projection,
  selectedFrame,
  selectedFrameId,
  commitInteraction,
}: PhotoGesturesInput) {
  const [zoomDraft, setZoomDraftState] = useState<ZoomDraft | null>(
    null,
  );
  const zoomDraftRef = useRef<ZoomDraft | null>(null);
  const [canvasPhotoPreview, setCanvasPhotoPreview] =
    useState<ScopedPhotoTransformPreview | null>(null);

  function setZoomDraft(next: ZoomDraft | null) {
    zoomDraftRef.current = next;
    setZoomDraftState(next);
  }

  useEffect(() => {
    setZoomDraft(null);
    setCanvasPhotoPreview(null);
  }, [projection.state.projectId]);

  function beginZoomGesture() {
    if (!selectedFrame?.photo) return;
    const projectId = projection.state.projectId;
    const frameId = selectedFrame.id;
    const currentValue = selectedFrame.photo.transform.userZoom;
    const currentDraft = zoomDraftRef.current;
    if (
      currentDraft?.projectId === projectId &&
      currentDraft.frameId === frameId &&
      !currentDraft.committing
    ) {
      return;
    }
    setZoomDraft({
      projectId,
      frameId,
      startValue: currentValue,
      value: currentValue,
      committing: false,
    });
  }

  function updateZoomGesture(nextValue: number) {
    if (!selectedFrame?.photo) return;
    const projectId = projection.state.projectId;
    const frameId = selectedFrame.id;
    const currentValue = selectedFrame.photo.transform.userZoom;
    const currentDraft = zoomDraftRef.current;
    const draft =
      currentDraft?.projectId === projectId &&
      currentDraft.frameId === frameId &&
      !currentDraft.committing
        ? currentDraft
        : {
            projectId,
            frameId,
            startValue: currentValue,
            value: currentValue,
            committing: false,
          };
    setZoomDraft({
      ...draft,
      value: nextValue,
    });
  }

  async function commitPhotoTransform(intent: ProjectIntent) {
    const committed = await commitInteraction(intent);
    if (!committed) {
      setCanvasPhotoPreview(null);
    }
    return committed;
  }

  async function finishZoomGesture() {
    if (!selectedFrame) return;
    const projectId = projection.state.projectId;
    const frameId = selectedFrame.id;
    const draft = zoomDraftRef.current;
    if (
      !draft ||
      draft.projectId !== projectId ||
      draft.frameId !== frameId ||
      draft.committing
    ) {
      return;
    }

    const delta = Number((draft.value - draft.startValue).toFixed(4));
    if (Math.abs(delta) < 0.0001) {
      setZoomDraft(null);
      return;
    }

    const committingDraft = { ...draft, committing: true };
    setZoomDraft(committingDraft);
    await commitPhotoTransform({
      kind: "transformPhoto",
      frameId,
      deltaPanX: 0,
      deltaPanY: 0,
      deltaZoom: delta,
    });
    if (zoomDraftRef.current === committingDraft) {
      setZoomDraft(null);
    }
  }

  useEffect(() => {
    if (
      zoomDraftRef.current &&
      (zoomDraftRef.current.projectId !==
        projection.state.projectId ||
        zoomDraftRef.current.frameId !== selectedFrameId)
    ) {
      setZoomDraft(null);
    }
    setCanvasPhotoPreview((current) =>
      current?.projectId === projection.state.projectId &&
      current.preview.frameId === selectedFrameId
        ? current
        : null,
    );
  }, [projection.state.projectId, selectedFrameId]);

  useEffect(() => {
    setCanvasPhotoPreview(null);
  }, [projection]);

  const selectedPhotoZoom =
    selectedFrame?.photo?.transform.userZoom ?? 1;
  const selectedCanvasPhotoPreview =
    canvasPhotoPreview?.projectId === projection.state.projectId &&
    canvasPhotoPreview.preview.frameId === selectedFrame?.id
      ? canvasPhotoPreview.preview
      : null;
  const displayedPhotoZoom =
    zoomDraft &&
    zoomDraft.projectId === projection.state.projectId &&
    zoomDraft.frameId === selectedFrame?.id
      ? zoomDraft.value
      : (selectedCanvasPhotoPreview?.zoom ?? selectedPhotoZoom);
  const displayedPhotoPanX =
    selectedCanvasPhotoPreview?.panX ??
    selectedFrame?.photo?.transform.panX ??
    0;

  return {
    displayedPhotoZoom,
    displayedPhotoPanX,
    zoomCommitting: Boolean(
      zoomDraft?.projectId === projection.state.projectId &&
        zoomDraft.committing,
    ),
    photoZoomPreview:
      zoomDraft?.projectId === projection.state.projectId
        ? {
            frameId: zoomDraft.frameId,
            value: zoomDraft.value,
          }
        : null,
    onTransformPreview: (preview: PhotoTransformPreview | null) =>
      setCanvasPhotoPreview(
        preview
          ? {
              projectId: projection.state.projectId,
              preview,
            }
          : null,
      ),
    onTransformCommit: (delta: {
      frameId: string;
      deltaPanX: number;
      deltaPanY: number;
      deltaZoom: number;
    }) =>
      commitPhotoTransform({
        kind: "transformPhoto",
        ...delta,
      }),
    beginZoomGesture,
    updateZoomGesture,
    finishZoomGesture,
  };
}
