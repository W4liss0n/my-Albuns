import type { EditorProjection, PhotoOrientationAction } from "../domain/project";
import corpus from "../../tests/fixtures/photo-orientation-cases.json";

// Generated and checked through the public ProjectCore edit boundary.
export const photoOrientationCorpus = corpus as unknown as {
  states: Record<string, EditorProjection>;
  transitions: { from: string; to: string; frameIds: string[]; action: PhotoOrientationAction }[];
  single: string[];
  group: string[];
  placeholders: string[];
};
