import type { ComposedFrame, EditorProjection, FrameStyleEdit } from "../domain/project";
import corpus from "../../tests/fixtures/frame-style-cases.json";

// Generated and checked through the public ProjectCore edit boundary.
export const frameStyleCorpus = corpus as unknown as {
  states: Record<string, EditorProjection>;
  previews: { from: string; edit: FrameStyleEdit; frames: ComposedFrame[] }[];
  transitions: { from: string; to: string; edit: FrameStyleEdit }[];
  single: string[];
  group: string[];
  placeholders: string[];
};
