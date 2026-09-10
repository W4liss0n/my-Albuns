import type { DecorativeDropPreview, DecorativeDropRequest, EditorProjection, ProjectIntent } from "../domain/project";
import corpus from "../../tests/fixtures/decorative-cases.json";

// Generated and verified through the public ProjectCore boundary.
export const decorativeCorpus = corpus as unknown as {
  states: Record<string, EditorProjection>;
  previews: { from: string; request: DecorativeDropRequest; preview: DecorativeDropPreview }[];
  transitions: { from: string; intent: Extract<ProjectIntent, { kind: "applyDecorative" }>; projection: EditorProjection }[];
};

export function decorativeStateName(projection: EditorProjection) {
  return Object.keys(decorativeCorpus.states).find((name) =>
    JSON.stringify(decorativeCorpus.states[name].state.album) === JSON.stringify(projection.state.album));
}

export function decorativePreview(projection: EditorProjection, request: DecorativeDropRequest) {
  const from = decorativeStateName(projection);
  const sample = decorativeCorpus.previews.find((item) => {
    const zone = item.preview.zoneRect;
    return item.from === from && item.request.sheetId === request.sheetId &&
      item.request.mediaId === request.mediaId && item.request.role === request.role &&
      request.xUm >= zone.x && request.xUm < zone.x + zone.width &&
      request.yUm >= zone.y && request.yUm < zone.y + zone.height;
  });
  return sample ? { ...structuredClone(sample.preview), revision: projection.state.revision } : null;
}
