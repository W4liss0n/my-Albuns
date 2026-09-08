import type { EditorProjection } from "../domain/project";
import corpus from "../../tests/fixtures/sheet-side-swap-cases.json";

// Captured and verified through the public ProjectCore boundary.
export const sheetSideSwapCorpus = corpus as unknown as {
  before: EditorProjection;
  cases: {
    name: string;
    targetSheetId: string;
    before?: EditorProjection;
    after: EditorProjection;
    outcome: "changed" | "unchanged" | "unavailable";
  }[];
};
