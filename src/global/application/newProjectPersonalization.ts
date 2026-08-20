import type {
  InitialBackgroundContent,
  InitialOverlayContent,
  InitialVisualDefaults,
  NewProjectConfiguration,
  NewProjectCreationConfiguration,
  ProvisionalDecorativeSelection,
} from "./globalProjectPort";

export type {
  NewProjectCreationConfiguration,
  ProvisionalDecorativeSelection,
} from "./globalProjectPort";

export type PersonalizationScope = "both" | "left" | "right";

export type BackgroundDraftContent =
  | { kind: "color"; rgb: string }
  | { kind: "image"; selection: ProvisionalDecorativeSelection };

export type OverlayDraftContent =
  | { kind: "image"; selection: ProvisionalDecorativeSelection }
  | null;

export type ScopedDraft<T> =
  | { scope: "bothSides"; both: T }
  | { scope: "perSide"; left: T; right: T };

export type FrameBorderDraft =
  | { kind: "none" }
  | { kind: "solid"; rgb: string; widthUm: number };

export interface NewProjectPersonalizationDraft {
  fixedScope: PersonalizationScope;
  background: ScopedDraft<BackgroundDraftContent>;
  overlay: ScopedDraft<OverlayDraftContent>;
  frameBorder: FrameBorderDraft;
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
  };
}

export function fixPersonalizationScope(
  draft: NewProjectPersonalizationDraft,
  fixedScope: PersonalizationScope,
): NewProjectPersonalizationDraft {
  return { ...draft, fixedScope };
}

export function backgroundForFixedScope(
  draft: NewProjectPersonalizationDraft,
): BackgroundDraftContent {
  return contentForScope(draft.background, draft.fixedScope);
}

export function setBackgroundColor(
  draft: NewProjectPersonalizationDraft,
  rgb: string,
): NewProjectPersonalizationDraft {
  return {
    ...draft,
    background: applyToScope(draft.background, draft.fixedScope, {
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
    background: applyToScope(draft.background, draft.fixedScope, {
      kind: "image",
      selection,
    }),
  };
}

export function overlayForFixedScope(
  draft: NewProjectPersonalizationDraft,
): OverlayDraftContent {
  return contentForScope(draft.overlay, draft.fixedScope);
}

export function setOverlayImage(
  draft: NewProjectPersonalizationDraft,
  selection: ProvisionalDecorativeSelection,
): NewProjectPersonalizationDraft {
  return {
    ...draft,
    overlay: applyToScope(draft.overlay, draft.fixedScope, {
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
    overlay: applyToScope(draft.overlay, draft.fixedScope, null),
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

export function setFrameBorderEnabled(
  draft: NewProjectPersonalizationDraft,
  enabled: boolean,
): NewProjectPersonalizationDraft {
  return {
    ...draft,
    frameBorder: enabled
      ? { kind: "solid", rgb: "#000000", widthUm: 1_000 }
      : { kind: "none" },
  };
}

export function setFrameBorderColor(
  draft: NewProjectPersonalizationDraft,
  rgb: string,
): NewProjectPersonalizationDraft {
  if (draft.frameBorder.kind === "none") {
    return draft;
  }
  return {
    ...draft,
    frameBorder: { ...draft.frameBorder, rgb: rgb.toUpperCase() },
  };
}

export function setFrameBorderWidth(
  draft: NewProjectPersonalizationDraft,
  widthUm: number,
): NewProjectPersonalizationDraft {
  if (draft.frameBorder.kind === "none" || !Number.isFinite(widthUm)) {
    return draft;
  }
  return {
    ...draft,
    frameBorder: {
      ...draft.frameBorder,
      widthUm: Math.max(1, Math.trunc(widthUm)),
    },
  };
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
  scoped: ScopedDraft<T>,
  map: (content: T) => U,
): ScopedDraft<U> {
  return scoped.scope === "bothSides"
    ? { scope: "bothSides", both: map(scoped.both) }
    : {
        scope: "perSide",
        left: map(scoped.left),
        right: map(scoped.right),
      };
}

function contentForScope<T>(
  scoped: ScopedDraft<T>,
  scope: PersonalizationScope,
): T {
  if (scoped.scope === "bothSides") {
    return scoped.both;
  }
  return scope === "right" ? scoped.right : scoped.left;
}

export function applyToScope<T>(
  scoped: ScopedDraft<T>,
  scope: PersonalizationScope,
  content: T,
): ScopedDraft<T> {
  if (scope === "both") {
    return { scope: "bothSides", both: content };
  }
  const left = contentForScope(scoped, "left");
  const right = contentForScope(scoped, "right");
  return scope === "left"
    ? { scope: "perSide", left: content, right }
    : { scope: "perSide", left, right: content };
}

function collectScoped<T>(
  scoped: ScopedDraft<T>,
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
