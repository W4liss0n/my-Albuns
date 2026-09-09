import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { photoOrientationCorpus as corpus } from "../test/photoOrientationPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function harness() {
  const initial = structuredClone(corpus.states.neutral);
  const rotated = structuredClone(corpus.states["single-rotated"]);
  const mirrored = structuredClone(corpus.states["single-both"]);
  const pending = deferred<EditorProjection>();
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported by this orientation test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>().mockImplementationOnce(() => pending.promise)
    .mockResolvedValue(mirrored);
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision },
    projection: { ...mirrored, state: { ...mirrored.state, savedRevision: revision, dirty: false } } }));
  const undo = vi.fn(async () => rotated);
  const port: ProjectCorePort = {
    load: async () => initial, apply, applyWithOutcome: unsupported, save, undo, redo: async () => mirrored,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }), readPhotoAngleDoubleClickTime: async () => 500,
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: unsupported,
    saveAs: unsupported, validateAlbumInformation: unsupported, importPhoto: unsupported,
    resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: corpus.single });
  const view = renderHook(({ blocked }) => {
    const [projection, setProjection] = useState(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    return { runner, projection, ...useProjectEditorController({ projection, projectCorePort: port,
      runProjectMutation: runner, onProjectionChange: setProjection, interactionBlocked: blocked }) };
  }, { initialProps: { blocked: false } });
  return { view, pending, apply, save, undo, initial, rotated, mirrored };
}

test.each(["success", "failure"])("orientation, adjacent orientation, Save and Undo share the queue: %s", async (outcome) => {
  const h = harness();
  act(() => {
    void h.view.result.current.orientPhotos("rotateCounterClockwise");
    void h.view.result.current.orientPhotos("toggleHorizontalMirror");
    h.view.result.current.save();
    h.view.result.current.undo();
  });
  expect(h.apply).toHaveBeenCalledTimes(1);
  expect(h.save).not.toHaveBeenCalled();
  expect(h.undo).not.toHaveBeenCalled();
  // Navigation changes do not redirect an already queued Photo command.
  act(() => useEditorView.getState().selectFrames(corpus.placeholders));
  await act(async () => {
    if (outcome === "success") h.pending.resolve(h.rotated);
    else h.pending.reject(new Error("Falha ao orientar as Fotos."));
    await h.view.result.current.runner.waitForIdle();
  });
  if (outcome === "success") {
    expect(h.apply).toHaveBeenNthCalledWith(2, { kind: "orientPhotos", frameIds: corpus.single,
      action: "toggleHorizontalMirror" }, expect.any(Function));
    expect(h.save).toHaveBeenCalledWith(h.mirrored.state.revision);
    expect(h.undo).toHaveBeenCalledOnce();
  } else {
    expect(h.apply).toHaveBeenCalledTimes(1);
    expect(h.save).not.toHaveBeenCalled();
    expect(h.undo).not.toHaveBeenCalled();
    expect(h.view.result.current.projection).toEqual(h.initial);
    expect(h.view.result.current.message).toBe("Falha ao orientar as Fotos.");
  }
  expect(useEditorView.getState().selectedFrameIds).toEqual(corpus.placeholders);
});

test("orientation is unavailable with no Photos or blocked interactions, and works in normal mode", async () => {
  const h = harness();
  h.view.rerender({ blocked: true });
  await act(async () => expect(await h.view.result.current.orientPhotos("rotateCounterClockwise")).toBe(false));
  h.view.rerender({ blocked: false });
  act(() => useEditorView.getState().selectFrames(corpus.placeholders));
  expect(h.view.result.current.canOrientPhotos).toBe(false);
  await act(async () => expect(await h.view.result.current.orientPhotos("toggleHorizontalMirror")).toBe(false));
  expect(h.apply).not.toHaveBeenCalled();
  act(() => useEditorView.setState({ editingSheetId: null, selectedFrameIds: corpus.single }));
  await act(async () => {
    const done = h.view.result.current.orientPhotos("rotateCounterClockwise");
    h.pending.resolve(h.rotated);
    expect(await done).toBe(true);
  });
  expect(useEditorView.getState().selectedFrameIds).toEqual(corpus.single);
});
