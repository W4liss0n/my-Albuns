import type { EditorProjection, PhotoDropTarget } from "../domain/project";
import corpus from "../../tests/fixtures/frame-content-swap-cases.json";

// Produced and checked through the public Core. JSON widens enum strings and
// palette tuples; restore the generated types at this fixture boundary.
export const frameContentSwapCorpus = corpus as unknown as {
  before: EditorProjection;
  cases: { name: string; selectedFrameIds: string[]; after: EditorProjection }[];
  dropProbes: { sheetId: string; xUm: number; yUm: number; target: PhotoDropTarget }[];
};
