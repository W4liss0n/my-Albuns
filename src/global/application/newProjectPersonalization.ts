import type {
  InitialBackgroundContent,
  InitialOverlayContent,
  InitialVisualDefaults,
  NewProjectConfiguration,
  NewProjectCreationConfiguration,
  ProvisionalDecorativeSelection,
} from "./globalProjectPort";
import type {
  FrameBorderValue,
  SolidFrameBorder,
} from "../../application/frameBorderEditor";
import {
  applyScopedValue,
  readScopedValue,
  type ScopedValue,
  type ScopedValueRead,
  type VisualScope,
} from "../../application/scopedValues";
import type { VisualPersonalizationPreview } from "../../ui/visualPreview";

export type {
  NewProjectCreationConfiguration,
  ProvisionalDecorativeSelection,
} from "./globalProjectPort";

export type BackgroundDraftContent =
  | { kind: "color"; rgb: string }
  | { kind: "image"; selection: ProvisionalDecorativeSelection };

export type OverlayDraftContent =
  | { kind: "image"; selection: ProvisionalDecorativeSelection }
  | null;

export interface NewProjectPersonalizationDraft {
  fixedScope: VisualScope;
  background: ScopedValue<BackgroundDraftContent>;
  overlay: ScopedValue<OverlayDraftContent>;
  frameBorder: FrameBorderValue;
  frameBorderPreference: SolidFrameBorder;
}

export function createDefaultPersonalizationDraft(): NewProjectPersonalizationDraft {
  return {
    fixedScope: "both",
    background: {
      scope: "bothSides",
      both: { kind: "color", rgb: "#FFFFFF" },
    },
    overlay: { scope: "bothSides", both: null },
    frameBorder: { kind: "none" },
    frameBorderPreference: { rgb: "#FFFFFF", widthUm: 1_000 },
  };
}

export function fixPersonalizationScope(
  draft: NewProjectPersonalizationDraft,
  fixedScope: VisualScope,
): NewProjectPersonalizationDraft {
  return { ...draft, fixedScope };
}

export function readBackgroundForFixedScope(
  draft: NewProjectPersonalizationDraft,
): ScopedValueRead<BackgroundDraftContent> {
  return readScopedValue(draft.background, draft.fixedScope, sameBackground);
}

export function setBackgroundColor(
  draft: NewProjectPersonalizationDraft,
  rgb: string,
): NewProjectPersonalizationDraft {
  return {
    ...draft,
    background: applyScopedValue(draft.background, draft.fixedScope, {
      kind: "color",
      rgb: rgb.toUpperCase(),
    }),
  };
}

export function setBackgroundImage(
  draft: NewProjectPersonalizationDraft,
  selection: ProvisionalDecorativeSelection,
): NewProjectPersonalizationDraft {
  return {
    ...draft,
    background: applyScopedValue(draft.background, draft.fixedScope, {
      kind: "image",
      selection,
    }),
  };
}

export function readOverlayForFixedScope(
  draft: NewProjectPersonalizationDraft,
): ScopedValueRead<OverlayDraftContent> {
  return readScopedValue(draft.overlay, draft.fixedScope, sameOverlay);
}

export function setOverlayImage(
  draft: NewProjectPersonalizationDraft,
  selection: ProvisionalDecorativeSelection,
): NewProjectPersonalizationDraft {
  return {
    ...draft,
    overlay: applyScopedValue(draft.overlay, draft.fixedScope, {
      kind: "image",
      selection,
    }),
  };
}

export function clearOverlay(
  draft: NewProjectPersonalizationDraft,
): NewProjectPersonalizationDraft {
  return {
    ...draft,
    overlay: applyScopedValue(draft.overlay, draft.fixedScope, null),
  };
}

export function provisionalSelections(
  draft: NewProjectPersonalizationDraft,
): readonly ProvisionalDecorativeSelection[] {
  const selections: ProvisionalDecorativeSelection[] = [];
  const seen = new Set<string>();
  const collect = (
    content: BackgroundDraftContent | OverlayDraftContent,
  ) => {
    if (
      content?.kind === "image" &&
      !seen.has(content.selection.selectionId)
    ) {
      seen.add(content.selection.selectionId);
      selections.push(content.selection);
    }
  };
  collectScoped(draft.background, collect);
  collectScoped(draft.overlay, collect);
  return selections;
}

export function toCreationConfiguration(
  dimensions: NewProjectConfiguration,
  personalization: NewProjectPersonalizationDraft,
): NewProjectCreationConfiguration {
  return {
    ...dimensions,
    visualDefaults: {
      background: mapScoped(personalization.background, mapBackground),
      overlay: mapScoped(personalization.overlay, mapOverlay),
      frameBorder: personalization.frameBorder,
    } satisfies InitialVisualDefaults,
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

function collectScoped<T>(
  scoped: ScopedValue<T>,
  collect: (content: T) => void,
) {
  if (scoped.scope === "bothSides") {
    collect(scoped.both);
    return;
  }
  collect(scoped.left);
  collect(scoped.right);
}

function mapBackground(content: BackgroundDraftContent): InitialBackgroundContent {
  return content.kind === "color"
    ? content
    : { kind: "image", selectionId: content.selection.selectionId };
}

function mapOverlay(content: OverlayDraftContent): InitialOverlayContent {
  return content
    ? { kind: "image", selectionId: content.selection.selectionId }
    : null;
}

export function personalizationPreviewFromDraft(
  draft: NewProjectPersonalizationDraft,
): VisualPersonalizationPreview {
  return {
    fixedScope: draft.fixedScope,
    background: mapScoped(draft.background, (content) =>
      content.kind === "color"
        ? content
        : { kind: "image", previewUrl: content.selection.previewUrl },
    ),
    overlay: mapScoped(draft.overlay, (content) =>
      content
        ? { kind: "image", previewUrl: content.selection.previewUrl }
        : null,
    ),
    frameBorder: draft.frameBorder,
  };
}

function sameBackground(
  left: BackgroundDraftContent,
  right: BackgroundDraftContent,
) {
  if (left.kind !== right.kind) return false;
  return left.kind === "color"
    ? right.kind === "color" && left.rgb === right.rgb
    : right.kind === "image" &&
        left.selection.selectionId === right.selection.selectionId;
}

function sameOverlay(left: OverlayDraftContent, right: OverlayDraftContent) {
  if (left === null || right === null) return left === right;
  return left.selection.selectionId === right.selection.selectionId;
}
