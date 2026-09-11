import type { ComposedFrame, FrameGeometryPreview } from "../domain/project";

export function frameGeometryPreview(frames: ComposedFrame[]): FrameGeometryPreview {
  return { frames, snap: { retained: [], guides: [] } };
}
