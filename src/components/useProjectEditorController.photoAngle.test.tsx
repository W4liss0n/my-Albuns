import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { ComposedFrame, EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { photoOrientationCorpus as corpus } from "../test/photoOrientationPreview";
import { PhotoAngleControl } from "./PhotoAngleControl";
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
  const initial = structuredClone(corpus.states.neutral);
  const first = structuredClone(corpus.states["single-angle"]);
  const second = structuredClone(corpus.states["single-angle-max"]);
  const pending = deferred<EditorProjection>();
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this Photo angle test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>().mockImplementationOnce(() => pending.promise).mockResolvedValue(second);
  const preview = vi.fn<ProjectCorePort["previewPhotoAngle"]>(async (edit) => {
    const sample = corpus.anglePreviews.find((item) => item.from === "neutral" && item.edit.angleTenths === edit.angleTenths);
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
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewPhotoAngle: preview, previewFrameGeometry: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, importPhoto: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: corpus.single });
  function useHarness(blocked = false) {
    const [projection, setProjection] = useState(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    const controller = useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port,
      runProjectMutation: runner, onProjectionChange: setProjection, interactionBlocked: blocked });
    return { runner, projection, setProjection, ...controller };
  }
  return { useHarness, port, pending, apply, preview, save, undo, initial, first, second };
}

test.each(["success", "failure"])("two Angle edits, Save and Undo share the authoritative queue: %s", async (outcome) => {
  const h = harness();
  const view = renderHook(() => h.useHarness());
  act(() => {
    view.result.current.photoAngle.onCommit(123);
    view.result.current.photoAngle.onCommit(450);
    void view.result.current.save();
    void view.result.current.undo();
  });
  expect(h.apply).toHaveBeenCalledTimes(1);
  expect(h.save).not.toHaveBeenCalled();
  expect(h.undo).not.toHaveBeenCalled();
  act(() => useEditorView.getState().selectFrames(corpus.placeholders));
  await act(async () => {
    if (outcome === "success") h.pending.resolve(h.first);
    else h.pending.reject(new Error("Falha ao ajustar Ângulo."));
    await view.result.current.runner.waitForIdle();
  });
  if (outcome === "success") {
    expect(h.apply).toHaveBeenNthCalledWith(2, { kind: "setPhotoAngle", edit: { frameIds: corpus.single, angleTenths: 450 } }, expect.any(Function));
    expect(h.save).toHaveBeenCalledWith(h.second.state.revision);
    expect(h.undo).toHaveBeenCalledOnce();
  } else {
    expect(h.apply).toHaveBeenCalledTimes(1);
    expect(h.save).not.toHaveBeenCalled();
    expect(h.undo).not.toHaveBeenCalled();
    expect(view.result.current.projection).toEqual(h.initial);
    expect(view.result.current.message).toBe("Falha ao ajustar Ângulo.");
  }
  expect(useEditorView.getState().selectedFrameIds).toEqual(corpus.placeholders);
});

test("a pending preview uses the latest response, creates no history and is discarded on reselection", async () => {
  const h = harness();
  const older = deferred<ComposedFrame[]>();
  const newer = deferred<ComposedFrame[]>();
  h.preview.mockImplementationOnce(() => older.promise).mockImplementationOnce(() => newer.promise);
  const view = renderHook(() => h.useHarness());
  await act(async () => view.result.current.photoAngle.onPreview(123));
  await act(async () => view.result.current.photoAngle.onPreview(450));
  const newestFrames = corpus.anglePreviews.find((item) => item.from === "neutral" && item.edit.angleTenths === 450)!.frames;
  await act(async () => newer.resolve(structuredClone(newestFrames)));
  expect(view.result.current.canvasProps.composition.sheets[0].frames[0]).toEqual(newestFrames[0]);
  await act(async () => older.resolve(h.first.composition.sheets[0].frames));
  expect(view.result.current.canvasProps.composition.sheets[0].frames[0]).toEqual(newestFrames[0]);
  expect(view.result.current.projection).toEqual(h.initial);
  expect(h.apply).not.toHaveBeenCalled();
  act(() => useEditorView.getState().selectFrames(corpus.placeholders));
  expect(view.result.current.canvasProps.composition).toEqual(h.initial.composition);
  expect(view.result.current.photoAngle.disabled).toBe(true);
  act(() => view.result.current.photoAngle.onCommit(123));
  expect(h.apply).not.toHaveBeenCalled();
});

test.each(["orientation", "black-and-white"])("a draft is flushed before an adjacent %s command and the preview waits for pending edits", async (command) => {
  const h = harness();
  const view = renderHook(() => h.useHarness());
  act(() => {
    view.result.current.photoAngle.onPreview(123);
    if (command === "orientation") void view.result.current.orientPhotos("rotateCounterClockwise");
    else void view.result.current.togglePhotoBlackAndWhite();
  });
  expect(h.apply).toHaveBeenCalledExactlyOnceWith({ kind: "setPhotoAngle", edit: { frameIds: corpus.single, angleTenths: 123 } }, expect.any(Function));
  await act(async () => view.result.current.photoAngle.onPreview(450));
  expect(h.preview).not.toHaveBeenCalled();
  await act(async () => { h.pending.resolve(h.first); await view.result.current.runner.waitForIdle(); });
  expect(h.apply).toHaveBeenNthCalledWith(2, command === "orientation"
    ? { kind: "orientPhotos", frameIds: corpus.single, action: "rotateCounterClockwise" }
    : { kind: "togglePhotoBlackAndWhite", frameIds: corpus.single }, expect.any(Function));
  expect(h.preview).toHaveBeenCalledWith({ frameIds: corpus.single, angleTenths: 450 });
});

test("composition changes request a fresh Core preview; failures and blocking clear it", async () => {
  const h = harness();
  const view = renderHook(({ blocked }) => h.useHarness(blocked), { initialProps: { blocked: false } });
  await act(async () => view.result.current.photoAngle.onPreview(123));
  const firstRequests = h.preview.mock.calls.length;
  await act(async () => view.result.current.setProjection(structuredClone(h.initial)));
  expect(h.preview).toHaveBeenCalledTimes(firstRequests + 1);
  h.preview.mockRejectedValueOnce(new Error("Prévia indisponível."));
  await act(async () => view.result.current.photoAngle.onPreview(450));
  expect(view.result.current.canvasProps.composition).toEqual(h.initial.composition);
  expect(view.result.current.message).toBe("Prévia indisponível.");
  await act(async () => view.result.current.photoAngle.onPreview(123));
  view.rerender({ blocked: true });
  expect(view.result.current.canvasProps.composition).toEqual(h.initial.composition);
  expect(h.apply).not.toHaveBeenCalled();
});

test("flushing from another surface settles the control timer; a failed commit restores its value", async () => {
  vi.useFakeTimers();
  const h = harness();
  function Panel() {
    const controller = h.useHarness();
    const angle = controller.projection.state.album.sheets[0].frames[0].photo!.transform.fineRotationDegrees;
    return <><PhotoAngleControl value={Math.round(angle * 10)} {...controller.photoAngle} />
      <button onClick={() => { void controller.save(); }}>Salvar teste</button></>;
  }
  render(<Panel />);
  await act(async () => {});
  const slider = screen.getByRole("slider");
  Object.defineProperty(slider, "setPointerCapture", { value: vi.fn() });
  fireEvent.pointerDown(slider, { pointerId: 1, button: 0 });
  fireEvent.change(slider, { target: { value: "12.3" } });
  fireEvent.pointerUp(slider, { pointerId: 1 });
  fireEvent.click(screen.getByRole("button", { name: "Salvar teste" }));
  await act(async () => vi.advanceTimersByTime(2_000));
  expect(h.apply).toHaveBeenCalledTimes(1);
  await act(async () => h.pending.reject(new Error("Falha ao gravar Ângulo.")));
  expect(screen.getByRole("spinbutton")).toHaveValue("0");
  expect(h.save).not.toHaveBeenCalled();
});
