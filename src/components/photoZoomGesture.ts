import type { NumberRange } from "../domain/project";

export interface PhotoZoomGesture {
  frameId: string;
  baseZoom: number;
  delta: number;
}

export interface PhotoZoomCommit {
  frameId: string;
  delta: number;
}

interface PhotoZoomStep {
  frameId: string;
  baseZoom: number;
  zoomRange: NumberRange;
  wheelDeltaY: number;
}

interface PhotoZoomTransition {
  gesture: PhotoZoomGesture;
  previewZoom: number;
  interruptedCommit: PhotoZoomCommit | null;
}

export function advancePhotoZoomGesture(
  current: PhotoZoomGesture | null,
  step: PhotoZoomStep,
): PhotoZoomTransition {
  const continuesCurrent = current?.frameId === step.frameId;
  const baseZoom = continuesCurrent
    ? current.baseZoom
    : clamp(step.baseZoom, step.zoomRange);
  const previousDelta = continuesCurrent ? current.delta : 0;
  const eventDelta = clamp(-step.wheelDeltaY * 0.0012, {
    minimum: -0.18,
    maximum: 0.18,
  });
  const delta = clamp(
    previousDelta + eventDelta,
    {
      minimum: step.zoomRange.minimum - baseZoom,
      maximum: step.zoomRange.maximum - baseZoom,
    },
  );

  return {
    gesture: {
      frameId: step.frameId,
      baseZoom,
      delta,
    },
    previewZoom: baseZoom + delta,
    interruptedCommit: continuesCurrent
      ? null
      : finishPhotoZoomGesture(current),
  };
}

export function finishPhotoZoomGesture(
  gesture: PhotoZoomGesture | null,
): PhotoZoomCommit | null {
  if (!gesture || Math.abs(gesture.delta) <= 0.0001) return null;
  return {
    frameId: gesture.frameId,
    delta: gesture.delta,
  };
}

function clamp(value: number, range: NumberRange) {
  return Math.min(range.maximum, Math.max(range.minimum, value));
}
