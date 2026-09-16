import type { ComposedFrame, EditorProjection, PhotoAngleEdit, PhotoZoomEdit, PhotoOrientationAction } from "../domain/project";
import corpus from "../../tests/fixtures/photo-orientation-cases.json";

// Generated and checked through the public ProjectCore edit boundary.
export const photoOrientationCorpus = corpus as unknown as {
  states: Record<string, EditorProjection>;
  transitions: { from: string; to: string; frameIds: string[]; action: PhotoOrientationAction }[];
  angleTransitions: { from: string; to: string; edit: PhotoAngleEdit }[];
  anglePreviews: { from: string; edit: PhotoAngleEdit; frames: ComposedFrame[] }[];
  zoomTransitions: { from: string; to: string; edit: PhotoZoomEdit }[];
  zoomPreviews: { from: string; edit: PhotoZoomEdit; frames: ComposedFrame[] }[];
  effectTransitions: { from: string; to: string; frameIds: string[] }[];
  single: string[];
  group: string[];
  placeholders: string[];
};
