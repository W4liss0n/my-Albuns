import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { ComposedFrame, EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { photoOrientationCorpus as corpus } from "../test/photoOrientationPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => {
  useEditorView.setState(useEditorView.getInitialState(), true);
  vi.useRealTimers();
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function harness() {
  const initial = structuredClone(corpus.states["single-zoom"]);
  const first = structuredClone(corpus.states["group-zoom"]);
  const second = structuredClone(corpus.states["group-zoom-two"]);
  const pending = deferred<EditorProjection>();
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this Photo Zoom test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>().mockImplementationOnce(() => pending.promise).mockResolvedValue(second);
  const preview = vi.fn<ProjectCorePort["previewPhotoZoom"]>(async (edit) => {
    const sample = corpus.zoomPreviews.find((item) => item.from === "single-zoom" && item.edit.userZoom === edit.userZoom);
    if (!sample) throw new Error("Missing Core preview sample.");
    return structuredClone(sample.frames);
  });
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision },
    projection: { ...second, state: { ...second.state, savedRevision: revision, dirty: false } } }));
  const undo = vi.fn(async () => first);
  const port: ProjectCorePort = {
    load: async () => initial, apply, applyWithOutcome: unsupported, save, undo, redo: async () => second,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }), readSliderDoubleClickTime: async () => 900,
    validateMediaFolderName: async () => { throw new Error("Folder validation is not configured in this fixture."); },
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewPhotoZoom: preview, previewFrameGeometry: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, importMedia: unsupported, resolvePhotoDropTarget: unsupported, replaceImage: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: corpus.group });
  function useHarness(blocked = false) {
    const [projection, setProjection] = useState(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    const controller = useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port,
      runProjectMutation: runner, onProjectionChange: setProjection, interactionBlocked: blocked });
    return { runner, projection, setProjection, ...controller };
  }
  return { useHarness, port, pending, apply, preview, save, undo, initial, first, second };
}

test.each(["success", "failure"])("two Zoom edits, Save and Undo share the authoritative queue: %s", async (outcome) => {
  const h = harness();
  const view = renderHook(() => h.useHarness());
  act(() => {
    view.result.current.photoZoom.onCommit(175);
    view.result.current.photoZoom.onCommit(200);
    void view.result.current.save();
    void view.result.current.undo();
  });
  expect(h.apply).toHaveBeenCalledTimes(1);
  expect(h.save).not.toHaveBeenCalled();
  expect(h.undo).not.toHaveBeenCalled();
  act(() => useEditorView.getState().selectFrames(corpus.placeholders));
  await act(async () => {
    if (outcome === "success") h.pending.resolve(h.first);
    else h.pending.reject(new Error("Falha ao ajustar Zoom."));
    await view.result.current.runner.waitForIdle();
  });
  if (outcome === "success") {
    expect(h.apply).toHaveBeenNthCalledWith(2, { kind: "setPhotoZoom", edit: { frameIds: corpus.group, userZoom: 2 } }, expect.any(Function));
    expect(h.save).toHaveBeenCalledWith(h.second.state.revision);
    expect(h.undo).toHaveBeenCalledOnce();
  } else {
    expect(h.apply).toHaveBeenCalledTimes(1);
    expect(h.save).not.toHaveBeenCalled();
    expect(h.undo).not.toHaveBeenCalled();
    expect(view.result.current.projection).toEqual(h.initial);
    expect(view.result.current.message).toBe("Falha ao ajustar Zoom.");
  }
  expect(useEditorView.getState().selectedFrameIds).toEqual(corpus.placeholders);
});

test("a pending preview uses the latest response, creates no history and is discarded on reselection", async () => {
  const h = harness();
  const older = deferred<ComposedFrame[]>();
  const newer = deferred<ComposedFrame[]>();
  h.preview.mockImplementationOnce(() => older.promise).mockImplementationOnce(() => newer.promise);
  const view = renderHook(() => h.useHarness());
  await act(async () => view.result.current.photoZoom.onPreview(175));
  await act(async () => view.result.current.photoZoom.onPreview(200));
  const newestFrames = corpus.zoomPreviews.find((item) => item.from === "single-zoom" && item.edit.userZoom === 2)!.frames;
  await act(async () => newer.resolve(structuredClone(newestFrames)));
  expect(view.result.current.canvasProps.composition.sheets[0].frames[0]).toEqual(newestFrames[0]);
  await act(async () => older.resolve(h.first.composition.sheets[0].frames));
  expect(view.result.current.canvasProps.composition.sheets[0].frames[0]).toEqual(newestFrames[0]);
  expect(view.result.current.projection).toEqual(h.initial);
  expect(h.apply).not.toHaveBeenCalled();
  act(() => useEditorView.getState().selectFrames(corpus.placeholders));
  expect(view.result.current.canvasProps.composition).toEqual(h.initial.composition);
  expect(view.result.current.photoZoom.disabled).toBe(true);
  act(() => view.result.current.photoZoom.onCommit(175));
  expect(h.apply).not.toHaveBeenCalled();
});

