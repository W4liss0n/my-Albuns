import { mapScopedValue } from "../application/scopedValues";
import type { VisualPersonalizationPreview } from "../ui/visualPreview";
import type {
  BackgroundDraftContent,
  NewProjectPersonalizationDraft,
  OverlayDraftContent,
} from "./application/newProjectPersonalization";

export function personalizationPreviewFromDraft(
  draft: NewProjectPersonalizationDraft,
): VisualPersonalizationPreview {
  return {
    fixedScope: draft.fixedScope,
    background: mapScopedValue(draft.background, mapBackground),
    overlay: mapScopedValue(draft.overlay, mapOverlay),
    frameBorder: draft.frameBorder,
  };
}

function mapBackground(content: BackgroundDraftContent) {
  return content.kind === "color"
    ? content
    : { kind: "image" as const, previewUrl: content.selection.previewUrl };
}

function mapOverlay(content: OverlayDraftContent) {
  return content
    ? { kind: "image" as const, previewUrl: content.selection.previewUrl }
    : null;
}
