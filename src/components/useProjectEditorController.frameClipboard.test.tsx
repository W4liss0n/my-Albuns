import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection, ProjectMutationOutcome } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { frameClipboardCorpus as corpus } from "../test/frameClipboardPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

test("select all and area selection stay in the edited Sheet, include placeholders and do not mutate the Project", async () => {
  const h = harness("same-group", "edit", true);
  const sheetId = useEditorView.getState().editingSheetId!;
  const ids = h.initial.state.album.sheets.find((sheet) => sheet.id === sheetId)!.frames.map((frame) => frame.id);
  act(() => useEditorView.getState().selectFrames([]));
  expect(h.view.result.current.canSelectAllFrames).toBe(true);
  act(() => h.view.result.current.selectAllFrames());
  expect(useEditorView.getState().selectedFrameIds).toEqual(ids);
  act(() => h.view.result.current.canvasProps.onSelectFrames?.([ids[0], "outside-sheet"]));
  expect(useEditorView.getState().selectedFrameIds).toEqual([ids[0]]);
  expect(h.apply).not.toHaveBeenCalled();
  expect(h.applyWithOutcome).not.toHaveBeenCalled();
  expect(h.view.result.current.projection.state).toEqual(h.initial.state);
  act(() => useEditorView.getState().exitSheetEdit());
  act(() => h.view.result.current.selectAllFrames());
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
  await act(async () => undefined);
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function harness(name = "same-group", mode: "edit" | "normal" = "edit", locked = false) {
  const initial = structuredClone(corpus.before);
  const copied = structuredClone(corpus.copied);
  const caseName = mode === "normal" && name === "same-single" ? "normal-same-single" : name;
  const scenario = corpus.cases.find((item) => item.name === caseName)!;
  initial.state.album.sheets.find((sheet) => sheet.id === scenario.sourceSheetId)!.layoutLocked = locked;
  copied.state.album.sheets.find((sheet) => sheet.id === scenario.sourceSheetId)!.layoutLocked = locked;
  const pasted = structuredClone(scenario.after);
  const pendingCopy = deferred<EditorProjection>();
  const pendingPaste = deferred<ProjectMutationOutcome>();
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this clipboard test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>(() => pendingCopy.promise);
  const applyWithOutcome = vi.fn<ProjectCorePort["applyWithOutcome"]>(() => pendingPaste.promise);
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision }, projection: { ...pasted, state: { ...pasted.state, dirty: false, savedRevision: revision } } }));
  const undo = vi.fn(async () => ({ ...copied, state: { ...copied.state, canRedo: true } }));
  const redo = vi.fn(async () => pasted);
  const port: ProjectCorePort = {
    load: async () => initial, apply, applyWithOutcome, save, undo, redo,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: unsupported, saveAs: unsupported, validateAlbumInformation: unsupported,
    importMedia: unsupported, resolvePhotoDropTarget: unsupported, replaceImage: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: mode === "edit" ? scenario.sourceSheetId : null,
    focusedSheetId: scenario.sourceSheetId, centeredSheetId: scenario.sourceSheetId, selectedFrameIds: scenario.selectedFrameIds });
  const view = renderHook(({ blocked, currentPort, projectId }) => {
    const [projection, setProjection] = useState(initial);
    const runProjectMutation = useProjectMutationRunner(projectId, currentPort);
    return { runner: runProjectMutation, projection, ...useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: currentPort,
      runProjectMutation, onProjectionChange: setProjection, interactionBlocked: blocked }) };
  }, { initialProps: { blocked: false, currentPort: port, projectId: initial.state.projectId } });
  // AlbumCanvasScene reports its dimensionless scale; geometry separately uses
  // 1 Canvas unit per 1,000 micrometers.
  act(() => view.result.current.canvasProps.onCanvasMetricsChange?.({ width: 1200, height: 500, scale: 2 }));
  return { view, initial, copied, pasted, scenario, pendingCopy, pendingPaste, apply, applyWithOutcome, save, undo, redo, port };
}

test.each((["edit", "normal"] as const).flatMap((mode) =>
  ["success", "copy-failure", "paste-failure"].map((result) => ({ mode, result }))))("rapid Copy, Paste, Save and Undo use the same authoritative queue: $mode/$result", async ({ mode, result }) => {
  const h = harness(mode === "normal" ? "same-single" : "same-group", mode);
  let commands!: Promise<unknown>;
  act(() => {
    commands = Promise.all([h.view.result.current.copyFrames(), h.view.result.current.pasteFrames()]);
    h.view.result.current.save();
    h.view.result.current.undo();
  });
  expect(h.applyWithOutcome).not.toHaveBeenCalled();
  expect(h.save).not.toHaveBeenCalled();
  await act(async () => {
    if (result === "copy-failure") h.pendingCopy.reject(new Error("Falha ao copiar."));
    else h.pendingCopy.resolve(h.copied);
    await Promise.resolve();
  });
  expect(h.apply.mock.calls[0][0]).toEqual({ kind: "copyFrames", frameIds: h.scenario.selectedFrameIds });
  if (result !== "copy-failure") {
    expect(h.applyWithOutcome.mock.calls[0][0]).toEqual({ kind: "pasteFrames", sheetId: h.scenario.targetSheetId, desiredOffsetUm: mode === "edit" ? 8000 : 0, mode });
    expect(h.view.result.current.projection.state).toEqual(h.initial.state);
    expect(h.view.result.current.canPasteFrames).toBe(true);
  }
  await act(async () => {
    if (result === "paste-failure") h.pendingPaste.reject(new Error("Falha ao colar."));
    else h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
    await commands;
    await h.view.result.current.runner.waitForIdle();
  });
  if (result === "success") {
    expect(h.save).toHaveBeenCalledWith(h.pasted.state.revision);
    expect(h.undo).toHaveBeenCalledOnce();
    expect(useEditorView.getState().selectedFrameIds).toEqual([]);
    expect(h.view.result.current.projection.state.album).toEqual(h.initial.state.album);
  } else {
    expect(h.save).not.toHaveBeenCalled();
    expect(h.undo).not.toHaveBeenCalled();
    expect(h.view.result.current.message).toMatch(/Falha ao/);
    expect(useEditorView.getState().selectedFrameIds).toEqual(h.scenario.selectedFrameIds);
  }
});

test("Copy stays available across Sheet navigation and Paste replaces an empty destination selection", async () => {
  const h = harness("right-double");
  await act(async () => { const copied = h.view.result.current.copyFrames(); h.pendingCopy.resolve(h.copied); await copied; });
  act(() => {
    useEditorView.getState().exitSheetEdit();
    useEditorView.getState().enterSheetEdit(h.scenario.targetSheetId);
  });
  expect(h.view.result.current.canCopyFrames).toBe(false);
  expect(h.view.result.current.canPasteFrames).toBe(true);
  await act(async () => {
    const pasted = h.view.result.current.pasteFrames();
    h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
    await pasted;
  });
  expect(h.applyWithOutcome.mock.calls[0][0]).toMatchObject({ sheetId: h.scenario.targetSheetId });
  expect(useEditorView.getState().selectedFrameIds).toEqual(h.scenario.pastedFrameIds);
  expect(h.view.result.current.selectedFrames).toHaveLength(2);
  await act(async () => { h.view.result.current.undo(); await h.view.result.current.runner.waitForIdle(); });
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
  await act(async () => { h.view.result.current.redo(); await h.view.result.current.runner.waitForIdle(); });
  expect(h.view.result.current.projection.state.album).toEqual(h.pasted.state.album);
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
});

test("a pending Paste never selects Frames in another isolated Sheet", async () => {
  const h = harness();
  await act(async () => { const copied = h.view.result.current.copyFrames(); h.pendingCopy.resolve(h.copied); await copied; });
  let pasted!: Promise<unknown>;
  act(() => {
    pasted = h.view.result.current.pasteFrames();
    useEditorView.getState().exitSheetEdit();
    useEditorView.getState().enterSheetEdit("sheet-003");
    useEditorView.getState().selectFrame("clipboard-frame-2-0");
  });
  await act(async () => {
    h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
    await pasted;
  });
  expect(useEditorView.getState().selectedFrameIds).toEqual(["clipboard-frame-2-0"]);
});

test("clipboard commands respect blocked interactions, an empty selection and an empty clipboard in both modes", async () => {
  const h = harness();
  expect(h.view.result.current.canPasteFrames).toBe(false);
  await act(async () => { await h.view.result.current.pasteFrames(); });
  h.view.rerender({ blocked: true, currentPort: h.port, projectId: h.initial.state.projectId });
  await act(async () => {
    expect(await h.view.result.current.copyFrames()).toBe(false);
    expect(await h.view.result.current.pasteFrames()).toBe(false);
  });
  h.view.rerender({ blocked: false, currentPort: h.port, projectId: h.initial.state.projectId });
  act(() => useEditorView.getState().exitSheetEdit());
  await act(async () => {
    expect(await h.view.result.current.copyFrames()).toBe(false);
    await h.view.result.current.pasteFrames();
  });
  expect(h.apply).not.toHaveBeenCalled();
  expect(h.applyWithOutcome).not.toHaveBeenCalled();
});

test("normal Copy uses the selection and Paste targets the centered Sheet", async () => {
  const h = harness("normal-other-single", "normal");
  act(() => useEditorView.getState().centerSheet("sheet-003"));
  expect(h.view.result.current.canCopyFrames).toBe(true);
  await act(async () => {
    const copied = h.view.result.current.copyFrames();
    h.pendingCopy.resolve(h.copied);
    await copied;
  });
  expect(h.view.result.current.canPasteFrames).toBe(true);
  await act(async () => {
    const pasted = h.view.result.current.pasteFrames();
    h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
    await pasted;
  });
  expect(h.apply.mock.calls[0][0]).toEqual({ kind: "copyFrames", frameIds: h.scenario.selectedFrameIds });
  expect(h.applyWithOutcome.mock.calls[0][0]).toMatchObject({ sheetId: "sheet-003", mode: "normal" });
  expect(useEditorView.getState().selectedFrameIds).toEqual(["pasted-frame-0"]);
  expect(useEditorView.getState().editingSheetId).toBeNull();
});

test("a group copied in Edit Mode can be pasted into the centered Sheet in Normal Mode with single selection", async () => {
  const h = harness("normal-other-double");
  await act(async () => {
    const copied = h.view.result.current.copyFrames();
    h.pendingCopy.resolve(h.copied);
    await copied;
  });
  act(() => {
    useEditorView.getState().exitSheetEdit();
    useEditorView.getState().centerSheet(h.scenario.targetSheetId);
  });
  expect(h.view.result.current.canCopyFrames).toBe(false);
  expect(h.view.result.current.canPasteFrames).toBe(true);
  await act(async () => {
    const pasted = h.view.result.current.pasteFrames();
    h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
    await pasted;
  });
  expect(h.applyWithOutcome.mock.calls[0][0]).toMatchObject({ sheetId: "sheet-003", mode: "normal" });
  expect(useEditorView.getState().selectedFrameIds).toEqual(["pasted-frame-1"]);
  expect(h.view.result.current.projection.state.album).toEqual(h.pasted.state.album);
  expect(useEditorView.getState().editingSheetId).toBeNull();
});

test.each([false, true])("normal Paste follows the Canvas center after scrolling, with cleared selection: %s", async (clearSelection) => {
  const h = harness("normal-other-single", "normal");
  await act(async () => {
    const copied = h.view.result.current.copyFrames();
    h.pendingCopy.resolve(h.copied);
    await copied;
  });
  act(() => {
    if (clearSelection) useEditorView.getState().selectFrame(null);
    h.view.result.current.canvasProps.onCenteredSheetChange(h.scenario.targetSheetId);
  });
  expect(useEditorView.getState().focusedSheetId).toBe(h.scenario.sourceSheetId);
  expect(useEditorView.getState().centeredSheetId).toBe(h.scenario.targetSheetId);
  await act(async () => {
    const pasted = h.view.result.current.pasteFrames();
    h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
    await pasted;
  });
  expect(h.applyWithOutcome.mock.calls[0][0]).toMatchObject({ sheetId: h.scenario.targetSheetId, mode: "normal" });
});

test.each(["selection", "navigation", "mode"])("pending normal Paste preserves a later %s change", async (change) => {
  const h = harness("same-single", "normal");
  await act(async () => {
    const copied = h.view.result.current.copyFrames();
    h.pendingCopy.resolve(h.copied);
    await copied;
  });
  let pasted!: Promise<unknown>;
  act(() => {
    pasted = h.view.result.current.pasteFrames();
    if (change === "selection") useEditorView.getState().selectFrame("clipboard-frame-1-1");
    else if (change === "navigation") {
      useEditorView.getState().selectFrame(null);
      useEditorView.getState().centerSheet("sheet-003");
    } else useEditorView.getState().enterSheetEdit("sheet-003");
  });
  const selection = useEditorView.getState().selectedFrameIds;
  await act(async () => {
    h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
    await pasted;
  });
  expect(useEditorView.getState().selectedFrameIds).toEqual(selection);
  expect(h.applyWithOutcome.mock.calls[0][0]).toMatchObject({ sheetId: "sheet-002" });
});

test.each(["edit", "normal"] as const)("a locked Sheet permits Copy and blocks Paste in %s mode", async (mode) => {
  const h = harness("same-single", mode, true);
  expect(h.view.result.current.canCopyFrames).toBe(true);
  await act(async () => {
    const copied = h.view.result.current.copyFrames();
    h.pendingCopy.resolve(h.copied);
    await copied;
  });
  expect(h.view.result.current.canPasteFrames).toBe(false);
  await act(async () => { expect(await h.view.result.current.pasteFrames()).toBe(false); });
  expect(h.applyWithOutcome).not.toHaveBeenCalled();
});

test.each((["edit", "normal"] as const).flatMap((mode) => [false, true].map((manualSelection) => ({ mode, manualSelection }))))(
  "two queued Pastes select the most recent copies unless the user selects again: $mode/$manualSelection", async ({ mode, manualSelection }) => {
  const h = harness("same-single", mode);
  const second = structuredClone(h.pasted);
  const sheet = second.state.album.sheets.find((item) => item.id === h.scenario.targetSheetId)!;
  const composed = second.composition.sheets.find((item) => item.sheetId === sheet.id)!;
  sheet.frames.push({ ...structuredClone(sheet.frames[sheet.frames.length - 1]), id: "second-pasted-frame", zIndex: sheet.frames.length });
  composed.frames.push({ ...structuredClone(composed.frames[composed.frames.length - 1]), frameId: "second-pasted-frame", zIndex: composed.frames.length });
  second.state.revision += 1;
  const pendingSecond = deferred<ProjectMutationOutcome>();
  h.applyWithOutcome.mockImplementationOnce(() => h.pendingPaste.promise).mockImplementationOnce(() => pendingSecond.promise);
  await act(async () => {
    const copied = h.view.result.current.copyFrames();
    h.pendingCopy.resolve(h.copied);
    await copied;
  });
  let pastes!: Promise<unknown>;
  act(() => { pastes = Promise.all([h.view.result.current.pasteFrames(), h.view.result.current.pasteFrames()]); });
  await act(async () => {
    h.pendingPaste.resolve({ projection: h.pasted, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: h.scenario.pastedFrameIds });
  });
  expect(useEditorView.getState().selectedFrameIds).toEqual(["pasted-frame-0"]);
  if (manualSelection) act(() => useEditorView.getState().selectFrame("pasted-frame-0"));
  await act(async () => {
    pendingSecond.resolve({ projection: second, affectedFrameId: null, affectedSheetId: null, affectedFrameIds: ["second-pasted-frame"] });
    await pastes;
  });
  expect(useEditorView.getState().selectedFrameIds).toEqual([manualSelection ? "pasted-frame-0" : "second-pasted-frame"]);
});

test("a replaced Project port discards a pending Copy and its adjacent Paste", async () => {
  const h = harness();
  let commands!: Promise<unknown>;
  act(() => { commands = Promise.all([h.view.result.current.copyFrames(), h.view.result.current.pasteFrames()]); });
  h.view.rerender({ blocked: false, currentPort: { ...h.port }, projectId: "other-project" });
  await act(async () => { h.pendingCopy.resolve(h.copied); await commands; });
  expect(h.applyWithOutcome).not.toHaveBeenCalled();
  expect(h.view.result.current.projection.canPasteFrames).toBe(false);
  expect(h.view.result.current.canPasteFrames).toBe(false);
});
