import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { frameStyleCorpus as corpus } from "../test/frameStylePreview";
import { useEditorView } from "../state/editorView";
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
  const initial = structuredClone(corpus.states.album);
  const first = structuredClone(corpus.states["single-opacity"]);
  const second = structuredClone(corpus.states["single-border"]);
  const pending = deferred<EditorProjection>();
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this Frame style test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>().mockImplementationOnce(() => pending.promise).mockResolvedValue(second);
  const preview = vi.fn<ProjectCorePort["previewFrameStyle"]>(async (edit) => {
    const sample = corpus.previews.find((item) => item.from === "album" &&
      item.edit.frameIds.join() === edit.frameIds.join() &&
      JSON.stringify(item.edit.change) === JSON.stringify(edit.change));
    if (!sample) throw new Error("Missing Core Frame-style preview sample.");
    return structuredClone(sample.frames);
  });
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision },
    projection: { ...second, state: { ...second.state, savedRevision: revision, dirty: false } } }));
  const undo = vi.fn(async () => first);
  const port: ProjectCorePort = {
    load: async () => initial, apply, applyWithOutcome: unsupported, save, undo, redo: async () => second,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }), readSliderDoubleClickTime: async () => 500,
    previewFrameStyle: preview, previewPhotoAngle: unsupported, previewFrameGeometry: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, importPhoto: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  const sheetId = initial.state.album.sheets[0].id;
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: sheetId,
    focusedSheetId: sheetId, centeredSheetId: sheetId, selectedFrameIds: corpus.single });
  function useHarness() {
    const [projection, setProjection] = useState(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    const controller = useProjectEditorController({ projection, projectCorePort: port,
      runProjectMutation: runner, onProjectionChange: setProjection });
    return { runner, projection, setProjection, ...controller };
  }
  return { useHarness, port, pending, apply, preview, save, undo, initial, first, second };
}

test.each(["success", "failure"])("Frame style edits, Save and Undo use the authoritative queue: %s", async (outcome) => {
  const h = harness();
  const view = renderHook(() => h.useHarness());
  act(() => {
    view.result.current.frameStyle.onCommit({ kind: "opacity", opacityPercent: 50 });
    view.result.current.frameStyle.onCommit({ kind: "borderWidth", widthUm: 5_000 });
    void view.result.current.save();
    void view.result.current.undo();
  });
  expect(h.apply).toHaveBeenCalledExactlyOnceWith({ kind: "setFrameStyle", edit: {
    frameIds: corpus.single, change: { kind: "opacity", opacityPercent: 50 },
  } }, expect.any(Function));
  act(() => useEditorView.getState().selectFrames(corpus.placeholders));
  await act(async () => {
    if (outcome === "success") h.pending.resolve(h.first);
    else h.pending.reject(new Error("Falha ao alterar estilo."));
    await view.result.current.runner.waitForIdle();
  });
  if (outcome === "success") {
    expect(h.apply).toHaveBeenNthCalledWith(2, { kind: "setFrameStyle", edit: {
      frameIds: corpus.single, change: { kind: "borderWidth", widthUm: 5_000 },
    } }, expect.any(Function));
    expect(h.save).toHaveBeenCalledWith(h.second.state.revision);
    expect(h.undo).toHaveBeenCalledOnce();
  } else {
    expect(h.apply).toHaveBeenCalledTimes(1);
    expect(h.save).not.toHaveBeenCalled();
    expect(h.undo).not.toHaveBeenCalled();
    expect(view.result.current.projection).toEqual(h.initial);
    expect(view.result.current.message).toBe("Falha ao alterar estilo.");
  }
  expect(useEditorView.getState().selectedFrameIds).toEqual(corpus.placeholders);
  expect(view.result.current.frameStyle.disabled).toBe(false);
});

test("committing another style property first preserves the current opacity draft", async () => {
  const h = harness();
  const view = renderHook(() => h.useHarness());
  await act(async () => view.result.current.frameStyle.onPreview({ kind: "opacity", opacityPercent: 50 }));
  expect(view.result.current.canvasProps.composition.sheets[0].frames[0].opacityByte).toBe(128);
  act(() => view.result.current.frameStyle.onCommit({ kind: "borderWidth", widthUm: 5_000 }));
  expect(h.apply.mock.calls[0][0]).toMatchObject({ edit: { change: { kind: "opacity", opacityPercent: 50 } } });
  await act(async () => { h.pending.resolve(h.first); await view.result.current.runner.waitForIdle(); });
  expect(h.apply.mock.calls[1][0]).toMatchObject({ edit: { change: { kind: "borderWidth", widthUm: 5_000 } } });
});

test.each(["cancel", "reselect"])("a late style preview cannot revive a draft after %s", async (action) => {
  const h = harness();
  const pending = deferred<Awaited<ReturnType<ProjectCorePort["previewFrameStyle"]>>>();
  h.preview.mockImplementationOnce(() => pending.promise);
  const view = renderHook(() => h.useHarness());
  await act(async () => view.result.current.frameStyle.onPreview({ kind: "opacity", opacityPercent: 50 }));
  act(() => {
    if (action === "cancel") view.result.current.frameStyle.onCancel();
    else useEditorView.getState().selectFrames(corpus.placeholders);
  });
  await act(async () => pending.resolve(h.first.composition.sheets[0].frames));
  expect(view.result.current.canvasProps.composition).toEqual(h.initial.composition);
  expect(h.apply).not.toHaveBeenCalled();
});
