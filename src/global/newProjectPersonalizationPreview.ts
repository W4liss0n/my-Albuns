import type { ScopedValue } from "../application/scopedValues";
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
    background: mapScoped(draft.background, mapBackground),
    overlay: mapScoped(draft.overlay, mapOverlay),
    frameBorder: draft.frameBorder,
  };
}

function mapScoped<T, U>(
  scoped: ScopedValue<T>,
  map: (content: T) => U,
): ScopedValue<U> {
  return scoped.scope === "bothSides"
    ? { scope: "bothSides", both: map(scoped.both) }
    : {
        scope: "perSide",
        left: map(scoped.left),
        right: map(scoped.right),
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
