import type { EditorProjection } from "../domain/project";
import corpus from "../../tests/fixtures/frame-clipboard-cases.json";

// Projections and placement results are produced and checked by the public Core.
export const frameClipboardCorpus = corpus as unknown as {
  before: EditorProjection;
  copied: EditorProjection;
  cases: { name: string; before?: EditorProjection; copied?: EditorProjection; sourceSheetId: string; targetSheetId: string;
    selectedFrameIds: string[]; pastedFrameIds: string[]; after: EditorProjection }[];
};
