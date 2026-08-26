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
    : {
        kind: "image" as const,
        preview: {
          state: "ready" as const,
          url: content.selection.previewUrl,
        },
      };
}

function mapOverlay(content: OverlayDraftContent) {
  return content
    ? {
        kind: "image" as const,
        preview: {
          state: "ready" as const,
          url: content.selection.previewUrl,
        },
      }
    : null;
}
