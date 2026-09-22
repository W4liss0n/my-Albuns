import { useEffect, useState } from "react";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import type { PhotoTransformPreview } from "./albumCanvasContract";

type ProjectedFrame =
  EditorProjection["state"]["album"]["sheets"][number]["frames"][number];

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
  const [canvasPhotoPreview, setCanvasPhotoPreview] =
    useState<ScopedPhotoTransformPreview | null>(null);

  async function commitPhotoTransform(intent: ProjectIntent) {
    const committed = await commitInteraction(intent);
    if (!committed) {
      setCanvasPhotoPreview(null);
    }
    return committed;
  }

  useEffect(() => {
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
  const displayedPhotoZoom = selectedCanvasPhotoPreview?.zoom ?? selectedPhotoZoom;

  return {
    displayedPhotoZoom,
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
  };
}
