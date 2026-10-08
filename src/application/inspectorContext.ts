import type {
  ComposedPhoto,
  ComposedSheet,
  FrameSnapshot,
  MediaCatalogItem,
} from "../domain/project";

/**
 * What the contextual panel describes: the canvas selection (Frames, the
 * edited Sheet or the Album) or the media panel selection.
 */
export type InspectorSubject = "canvas" | "media";

export type InspectorContext =
  | { kind: "album" }
  | { kind: "sheet"; sheet: ComposedSheet }
  | { kind: "multiple-frames"; frames: readonly FrameSnapshot[]; editingSheet: ComposedSheet }
  | {
      kind: "frame";
      frame: FrameSnapshot;
      composedPhoto: ComposedPhoto | null;
      editingSheet?: ComposedSheet;
    }
  | { kind: "media"; media: MediaCatalogItem; editingSheet?: ComposedSheet }
  | { kind: "multiple-media"; count: number; editingSheet?: ComposedSheet };

export interface MediaSelectionChange {
  readonly mediaIds: readonly string[];
  /**
   * A selection gesture in the media panel (click, Ctrl, Shift, Ctrl+A, right
   * click, click on the grid background). Pruning by a search, a filter, a
   * tab or a removal, and the selection requested after an import, are not.
   */
  readonly explicit: boolean;
}

/**
 * The last explicit selection gesture owns the panel. A media gesture claims
 * it; an automatic change never does. An empty media selection cannot be the
 * subject, so the canvas takes it back and a later automatic selection (after
 * an import) does not switch the panel by itself.
 */
export function inspectorSubjectAfterMediaSelection(
  current: InspectorSubject,
  change: MediaSelectionChange,
): InspectorSubject {
  if (change.mediaIds.length === 0) return "canvas";
  return change.explicit ? "media" : current;
}

/**
 * Chooses what the contextual panel shows. The media selection is shown only
 * while it is the subject and the media panel is on screen; otherwise the
 * canvas selection decides, as before the media panel could own the panel.
 */
export function resolveInspectorContext({
  subject,
  mediaPanelVisible,
  selectedMedia,
  selectedFrames,
  composedPhoto,
  editingSheet,
}: {
  readonly subject: InspectorSubject;
  readonly mediaPanelVisible: boolean;
  /** The media panel selection, in visible order. */
  readonly selectedMedia: readonly MediaCatalogItem[];
  readonly selectedFrames: readonly FrameSnapshot[];
  /** The Photo of the single selected Frame, when it has one. */
  readonly composedPhoto: ComposedPhoto | null;
  readonly editingSheet: ComposedSheet | null;
}): InspectorContext {
  const editing = editingSheet ? { editingSheet } : {};
  if (subject === "media" && mediaPanelVisible && selectedMedia.length > 0) {
    return selectedMedia.length === 1
      ? { kind: "media", media: selectedMedia[0], ...editing }
      : { kind: "multiple-media", count: selectedMedia.length, ...editing };
  }
  if (editingSheet && selectedFrames.length > 1) {
    return { kind: "multiple-frames", frames: selectedFrames, editingSheet };
  }
  if (selectedFrames.length === 1) {
    return { kind: "frame", frame: selectedFrames[0], composedPhoto, ...editing };
  }
  return editingSheet ? { kind: "sheet", sheet: editingSheet } : { kind: "album" };
}

/**
 * The media whose whole preview the panel shows, if any. A Frame without a
 * Photo, several Frames and several images have no single preview.
 */
export function inspectorPreviewMediaId(context: InspectorContext): string | null {
  if (context.kind === "frame") return context.frame.photo?.mediaId ?? null;
  if (context.kind === "media") return context.media.id;
  return null;
}
