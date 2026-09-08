import type { EditorProjection } from "../domain/project";
import corpus from "../../tests/fixtures/frame-deletion-cases.json";

// Produced and checked by the public Core deletion tests. JSON imports widen
// enum strings and palette tuples; this boundary restores the generated types.
export const frameDeletionCorpus = corpus as unknown as {
  before: EditorProjection;
  cases: { name: string; selectedFrameIds: string[]; after: EditorProjection }[];
};
