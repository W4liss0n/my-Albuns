// @vitest-environment node
import { describe, expect, test } from "vitest";

import type {
  ComposedPhoto,
  ComposedSheet,
  FrameSnapshot,
  MediaCatalogItem,
} from "../domain/project";
import {
  inspectorPreviewMediaId,
  inspectorSubjectAfterMediaSelection,
  resolveInspectorContext,
  type InspectorSubject,
} from "./inspectorContext";

// The resolver only routes these values; it never reads their geometry.
const sheet = { sheetId: "sheet-001", number: 1 } as ComposedSheet;
const photoFrame = { id: "frame-001", photo: { mediaId: "media-001" } } as FrameSnapshot;
const emptyFrame = { id: "frame-empty", photo: null } as FrameSnapshot;
const composedPhoto = { mediaId: "media-001", name: "Serra ao amanhecer.jpg" } as ComposedPhoto;
const landscape = photo("media-001", "Serra ao amanhecer.jpg", 6_000, 4_000);
const portrait = photo("media-002", "Campo.jpg", 4_000, 6_000);

function photo(id: string, name: string, width: number, height: number): MediaCatalogItem {
  return { id, kind: "photo", name, palette: null, sourceWidthPx: width, sourceHeightPx: height };
}

function context(input: {
  subject?: InspectorSubject;
  mediaPanelVisible?: boolean;
  selectedMedia?: readonly MediaCatalogItem[];
  selectedFrames?: readonly FrameSnapshot[];
  editing?: boolean;
}) {
  return resolveInspectorContext({
    subject: input.subject ?? "canvas",
    mediaPanelVisible: input.mediaPanelVisible ?? true,
    selectedMedia: input.selectedMedia ?? [],
    selectedFrames: input.selectedFrames ?? [],
    composedPhoto: input.selectedFrames?.length === 1 && input.selectedFrames[0].photo ? composedPhoto : null,
    editingSheet: input.editing ? sheet : null,
  });
}

describe("the last explicit selection gesture owns the contextual panel", () => {
  test("a media gesture claims the panel, an empty selection returns it to the canvas", () => {
    expect(inspectorSubjectAfterMediaSelection("canvas", { mediaIds: ["media-001"], explicit: true })).toBe("media");
    expect(inspectorSubjectAfterMediaSelection("media", { mediaIds: ["media-001", "media-002"], explicit: true })).toBe("media");
    // A click on the grid background, or Ctrl on the last selected image.
    expect(inspectorSubjectAfterMediaSelection("media", { mediaIds: [], explicit: true })).toBe("canvas");
  });

  test("pruning and the selection after an import never claim the panel", () => {
    expect(inspectorSubjectAfterMediaSelection("canvas", { mediaIds: ["media-003"], explicit: false })).toBe("canvas");
    expect(inspectorSubjectAfterMediaSelection("media", { mediaIds: ["media-001"], explicit: false })).toBe("media");
    // A filter that hides every selected image leaves nothing to describe.
    expect(inspectorSubjectAfterMediaSelection("media", { mediaIds: [], explicit: false })).toBe("canvas");
  });
});

describe("resolveInspectorContext", () => {
  test("the media subject replaces the Album, the Sheet and a selected Frame", () => {
    expect(context({ subject: "media", selectedMedia: [landscape] })).toEqual({ kind: "media", media: landscape });
    expect(context({ subject: "media", selectedMedia: [landscape], editing: true }))
      .toEqual({ kind: "media", media: landscape, editingSheet: sheet });
    expect(context({ subject: "media", selectedMedia: [portrait], selectedFrames: [photoFrame] }))
      .toEqual({ kind: "media", media: portrait });
    expect(context({ subject: "media", selectedMedia: [landscape, portrait], selectedFrames: [photoFrame, emptyFrame], editing: true }))
      .toEqual({ kind: "multiple-media", count: 2, editingSheet: sheet });
  });

  test("the canvas subject ignores a media selection that is still on screen", () => {
    expect(context({ selectedMedia: [landscape], selectedFrames: [photoFrame] }))
      .toEqual({ kind: "frame", frame: photoFrame, composedPhoto });
    expect(context({ selectedMedia: [landscape], selectedFrames: [photoFrame, emptyFrame], editing: true }))
      .toEqual({ kind: "multiple-frames", frames: [photoFrame, emptyFrame], editingSheet: sheet });
    // A tap on a Sheet or outside the Sheets clears the Frames and claims the panel.
    expect(context({ selectedMedia: [landscape] })).toEqual({ kind: "album" });
    expect(context({ selectedMedia: [landscape], editing: true })).toEqual({ kind: "sheet", sheet });
  });

  test("a media subject without an image on screen falls back to the canvas", () => {
    expect(context({ subject: "media", selectedFrames: [emptyFrame], editing: true }))
      .toEqual({ kind: "frame", frame: emptyFrame, composedPhoto: null, editingSheet: sheet });
    expect(context({ subject: "media", mediaPanelVisible: false, selectedMedia: [landscape] })).toEqual({ kind: "album" });
    expect(context({ subject: "media", mediaPanelVisible: false, selectedMedia: [landscape], editing: true }))
      .toEqual({ kind: "sheet", sheet });
  });

  test("keeps the canvas rules: several Frames outside the Sheet editing show the Album", () => {
    expect(context({ selectedFrames: [photoFrame, emptyFrame] })).toEqual({ kind: "album" });
  });
});

test("only a Photo Frame and a single image have a whole preview", () => {
  expect(inspectorPreviewMediaId(context({ selectedFrames: [photoFrame] }))).toBe("media-001");
  expect(inspectorPreviewMediaId(context({ selectedFrames: [emptyFrame], editing: true }))).toBeNull();
  expect(inspectorPreviewMediaId(context({ selectedFrames: [photoFrame, emptyFrame], editing: true }))).toBeNull();
  expect(inspectorPreviewMediaId(context({ subject: "media", selectedMedia: [portrait] }))).toBe("media-002");
  expect(inspectorPreviewMediaId(context({ subject: "media", selectedMedia: [landscape, portrait] }))).toBeNull();
  expect(inspectorPreviewMediaId(context({}))).toBeNull();
});
