import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../../application/projectPorts";
import type { EditorProjection } from "../../domain/project";
import { useEditorView } from "../../state/editorView";
import { frameContentSwapCorpus as corpus } from "../../test/frameContentSwapPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

function swapHarness() {
  const initial = structuredClone(corpus.before);
  const swap = corpus.cases.find((item) => item.name === "photos")!;
  const swapped = structuredClone(swap.after);
  let resolve!: (projection: EditorProjection) => void;
  let reject!: (error: Error) => void;
  const pending = new Promise<EditorProjection>((yes, no) => { resolve = yes; reject = no; });
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this swap test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>(() => pending);
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision },
    projection: { ...swapped, state: { ...swapped.state, savedRevision: revision, dirty: false } } }));
  const undo = vi.fn(async () => ({ ...initial,
    state: { ...initial.state, revision: swapped.state.revision + 1, canRedo: true } }));
  const redo = vi.fn(async () => ({ ...swapped,
    state: { ...swapped.state, revision: swapped.state.revision + 2 } }));
  const port: ProjectCorePort = {
    load: async () => initial, apply, save, undo, redo,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    validateMediaFolderName: async () => { throw new Error("Folder validation is not configured in this fixture."); },
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Quadro style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoZoom: async () => { throw new Error("Photo zoom preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, applyWithOutcome: unsupported,
    importMedia: unsupported, resolvePhotoDropTarget: unsupported, replaceImage: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: swap.selectedFrameIds });
  const view = renderHook(({ blocked }) => {
    const [projection, setProjection] = useState(initial);
    const runProjectMutation = useProjectMutationRunner(initial.state.projectId, port);
    return useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port,
      runProjectMutation, onProjectionChange: setProjection, interactionBlocked: blocked });
  }, { initialProps: { blocked: false } });
  return { view, initial, swapped, resolve, reject, apply, save, undo, redo, port };
}

test.each([false, true])("normal drop serializes Core resolution, swap and adjacent Save/Undo; failure=%s", async (fails) => {
  const h = swapHarness();
  let resolveTarget!: (target: Awaited<ReturnType<ProjectCorePort["resolvePhotoDropTarget"]>>) => void;
  h.port.resolvePhotoDropTarget = vi.fn<ProjectCorePort["resolvePhotoDropTarget"]>(() => new Promise((yes) => { resolveTarget = yes; }));
  act(() => useEditorView.getState().exitSheetEdit());
  let completion!: Promise<boolean>;
  act(() => {
    completion = h.view.result.current.canvasProps.frameContentSwap!.commit("swap-frame-0", { sheetId: "sheet-002", xUm: 100, yUm: 200 });
    h.view.result.current.save(); h.view.result.current.undo();
    useEditorView.getState().selectFrame("swap-frame-3");
  });
  await waitFor(() => expect(h.port.resolvePhotoDropTarget).toHaveBeenCalledWith("sheet-002", 100, 200));
  expect(h.apply).not.toHaveBeenCalled(); expect(h.save).not.toHaveBeenCalled(); expect(h.undo).not.toHaveBeenCalled();
  await act(async () => {
    resolveTarget({ kind: "frame", frameId: "swap-frame-4" });
    if (fails) h.reject(new Error("A troca falhou.")); else h.resolve(h.swapped);
    await completion;
  });
  expect(h.apply.mock.calls[0][0]).toEqual({ kind: "swapFrameContents", frameIds: ["swap-frame-0", "swap-frame-4"] });
  if (fails) {
    expect(h.save).not.toHaveBeenCalled(); expect(h.undo).not.toHaveBeenCalled();
    expect(h.view.result.current.message).toBe("A troca falhou.");
  } else {
    expect(h.save).toHaveBeenCalledWith(h.swapped.state.revision); expect(h.undo).toHaveBeenCalledOnce();
  }
  expect(useEditorView.getState().selectedFrameIds).toEqual(["swap-frame-3"]);
});

test.each(["self", "source-sheet-background", "invalid", "empty-source", "blocked", "editing"])("normal drop on %s does not create a History command", async (reason) => {
  const h = swapHarness();
  h.port.applyWithOutcome = vi.fn<ProjectCorePort["applyWithOutcome"]>();
  h.port.resolvePhotoDropTarget = vi.fn<ProjectCorePort["resolvePhotoDropTarget"]>(async () => reason === "self" ? { kind: "frame", frameId: "swap-frame-0" }
    : reason === "source-sheet-background" ? { kind: "sheet", sheetId: "sheet-001" } : { kind: "invalid" });
  if (reason !== "editing") act(() => useEditorView.getState().exitSheetEdit());
  if (reason === "blocked") h.view.rerender({ blocked: true });
  await act(async () => {
    expect(await h.view.result.current.canvasProps.frameContentSwap!.commit(reason === "empty-source" ? "swap-frame-2" : "swap-frame-0",
      { sheetId: "sheet-002", xUm: 100, yUm: 200 })).toBe(false);
  });
  expect(h.apply).not.toHaveBeenCalled();
  expect(h.port.applyWithOutcome).not.toHaveBeenCalled();
});

test.each([true, false])("a drop on the free area of another Sheet moves the Photo in one command; source selected=%s", async (sourceSelected) => {
  const h = swapHarness();
  const moved = structuredClone(h.initial);
  const [origin, destination] = moved.state.album.sheets;
  const source = origin.frames.find((frame) => frame.id === "swap-frame-0")!;
  origin.frames = origin.frames.filter((frame) => frame !== source);
  destination.frames.push({ ...source, id: "moved-frame" });
  moved.state.revision += 1;
  h.port.resolvePhotoDropTarget = vi.fn<ProjectCorePort["resolvePhotoDropTarget"]>(async () => ({ kind: "sheet", sheetId: "sheet-002" }));
  h.port.applyWithOutcome = vi.fn<ProjectCorePort["applyWithOutcome"]>(async () => ({
    projection: moved, affectedFrameId: "moved-frame", affectedSheetId: "sheet-002" }));
  act(() => {
    useEditorView.getState().exitSheetEdit();
    useEditorView.getState().selectFrame(sourceSelected ? "swap-frame-0" : "swap-frame-3");
  });
  await act(async () => {
    expect(await h.view.result.current.canvasProps.frameContentSwap!.commit("swap-frame-0",
      { sheetId: "sheet-002", xUm: 100, yUm: 200 })).toBe(true);
  });
  expect(h.port.applyWithOutcome).toHaveBeenCalledExactlyOnceWith(
    { kind: "moveFrameToSheet", frameId: "swap-frame-0", sheetId: "sheet-002", xUm: 100, yUm: 200 }, expect.any(Function));
  expect(h.apply).not.toHaveBeenCalled();
  // The moved Photo keeps the selection it had; another selection stays put.
  expect(useEditorView.getState().selectedFrameIds).toEqual([sourceSelected ? "moved-frame" : "swap-frame-3"]);
});

test("a pending command that changes the source Photo cancels the queued normal drop", async () => {
  const h = swapHarness();
  let first!: Promise<boolean>;
  act(() => { first = h.view.result.current.swapFrameContents(); useEditorView.getState().exitSheetEdit(); });
  let drop!: Promise<boolean>;
  act(() => { drop = h.view.result.current.canvasProps.frameContentSwap!.commit("swap-frame-0", { sheetId: "sheet-002", xUm: 100, yUm: 200 }); });
  await act(async () => { h.resolve(h.swapped); await first; expect(await drop).toBe(false); });
  expect(h.apply).toHaveBeenCalledOnce();
});

test.each([false, true])("swap captures the pair and queues Save/Undo while preserving a later selection; failure=%s", async (fails) => {
  const harness = swapHarness();
  const { view, apply, save, undo, swapped } = harness;
  let completion!: Promise<unknown>;
  act(() => {
    completion = Promise.all([view.result.current.swapFrameContents(), view.result.current.save(), view.result.current.undo()]);
    useEditorView.getState().selectFrame("swap-frame-3");
  });
  expect(save).not.toHaveBeenCalled();
  expect(undo).not.toHaveBeenCalled();
  await act(async () => {
    if (fails) harness.reject(new Error("A troca falhou."));
    else harness.resolve(swapped);
    await completion;
  });
  expect(apply.mock.calls[0][0]).toEqual({ kind: "swapFrameContents", frameIds: ["swap-frame-0", "swap-frame-1"] });
  if (fails) {
    expect(save).not.toHaveBeenCalled();
    expect(undo).not.toHaveBeenCalled();
    expect(view.result.current.message).toBe("A troca falhou.");
  } else {
    expect(save).toHaveBeenCalledWith(swapped.state.revision);
    expect(undo).toHaveBeenCalledOnce();
  }
  expect(useEditorView.getState().selectedFrameIds).toEqual(["swap-frame-3"]);
  expect(view.result.current.selectedFrames.map((frame) => frame.id)).toEqual(["swap-frame-3"]);
});

test("swap, Undo and Redo preserve both selected Frames and show the corresponding Photos", async () => {
  const { view, initial, swapped, resolve } = swapHarness();
  await act(async () => {
    const completion = view.result.current.swapFrameContents();
    resolve(swapped);
    await completion;
  });
  const selectedPhotos = () => view.result.current.selectedFrames.map((frame) => frame.photo);
  expect(view.result.current.canSwapFrameContents).toBe(true);
  expect(selectedPhotos()).toEqual(swapped.state.album.sheets[0].frames.slice(0, 2).map((frame) => frame.photo));
  await act(async () => { await view.result.current.undo(); });
  expect(selectedPhotos()).toEqual(initial.state.album.sheets[0].frames.slice(0, 2).map((frame) => frame.photo));
  await act(async () => { await view.result.current.redo(); });
  expect(selectedPhotos()).toEqual(swapped.state.album.sheets[0].frames.slice(0, 2).map((frame) => frame.photo));
  expect(useEditorView.getState().selectedFrameIds).toEqual(["swap-frame-0", "swap-frame-1"]);
});

test("swap requires exactly two selected Frames with a Photo, editing mode and an unblocked interaction", async () => {
  const { view, apply } = swapHarness();
  view.rerender({ blocked: true });
  await act(async () => { expect(await view.result.current.swapFrameContents()).toBe(false); });
  view.rerender({ blocked: false });
  for (const selectedFrameIds of [[], ["swap-frame-0"], ["swap-frame-0", "swap-frame-1", "swap-frame-2"], ["swap-frame-2", "swap-frame-3"]]) {
    act(() => useEditorView.setState({ selectedFrameIds }));
    expect(view.result.current.canSwapFrameContents).toBe(false);
    await act(async () => { expect(await view.result.current.swapFrameContents()).toBe(false); });
  }
  act(() => useEditorView.setState({ selectedFrameIds: ["swap-frame-0", "swap-frame-2"] }));
  expect(view.result.current.canSwapFrameContents).toBe(true);
  act(() => useEditorView.getState().exitSheetEdit());
  await act(async () => { expect(await view.result.current.swapFrameContents()).toBe(false); });
  expect(apply).not.toHaveBeenCalled();
});
