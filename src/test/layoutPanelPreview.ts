import type { ComposedFrame, EditorProjection, LayoutExportProblem, LayoutQueryResult, SaveCustomLayoutResult } from "../domain/project";
import corpus from "../../tests/fixtures/layout-panel-cases.json";

interface LayoutPanelSample {
  projection: EditorProjection;
  queries: Record<string, { query: LayoutQueryResult; previews: ComposedFrame[][] }>;
}

// This visual adapter replays the public Core corpus; it never computes Layout geometry.
export const layoutPanelCorpus = corpus as unknown as {
  cases: Record<string, { before: LayoutPanelSample; applied: LayoutPanelSample | null;
    favoriteStates?: Record<string, LayoutPanelSample>; favoriteTransitions?: { from: string; to: string; candidateIndex: number }[];
    catalogSaved?: LayoutPanelSample; catalogDeleted?: LayoutPanelSample; saveResult?: SaveCustomLayoutResult;
    lockReady?: LayoutPanelSample; locked?: LayoutPanelSample; unlocked?: LayoutPanelSample;
    filled?: LayoutPanelSample; cleared?: LayoutPanelSample; exportProblems?: LayoutExportProblem[] }>;
};
