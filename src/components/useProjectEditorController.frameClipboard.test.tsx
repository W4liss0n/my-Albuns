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

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function harness(name = "same-group") {
  const initial = structuredClone(corpus.before);
  const copied = structuredClone(corpus.copied);
  const scenario = corpus.cases.find((item) => item.name === name)!;
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
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    previewFrameGeometry: unsupported, saveAs: unsupported, validateAlbumInformation: unsupported,
    importPhoto: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: scenario.sourceSheetId,
    focusedSheetId: scenario.sourceSheetId, centeredSheetId: scenario.sourceSheetId, selectedFrameIds: scenario.selectedFrameIds });
  const view = renderHook(({ blocked, currentPort, projectId }) => {
    const [projection, setProjection] = useState(initial);
    const runProjectMutation = useProjectMutationRunner(projectId, currentPort);
    return { runner: runProjectMutation, projection, ...useProjectEditorController({ projection, projectCorePort: currentPort,
      runProjectMutation, onProjectionChange: setProjection, interactionBlocked: blocked }) };
  }, { initialProps: { blocked: false, currentPort: port, projectId: initial.state.projectId } });
  // AlbumCanvasScene reports its dimensionless scale; geometry separately uses
  // 1 Canvas unit per 1,000 micrometers.
  act(() => view.result.current.canvasProps.onCanvasMetricsChange?.({ width: 1200, scale: 2 }));
  return { view, initial, copied, pasted, scenario, pendingCopy, pendingPaste, apply, applyWithOutcome, save, undo, redo, port };
}

test.each(["success", "copy-failure", "paste-failure"])("rapid Copy, Paste, Save and Undo use the same authoritative queue: %s", async (result) => {
  const h = harness();
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
    expect(h.applyWithOutcome.mock.calls[0][0]).toEqual({ kind: "pasteFrames", sheetId: h.scenario.targetSheetId, desiredOffsetUm: 8000 });
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

test("clipboard commands respect editing mode, blocked interactions and an empty clipboard", async () => {
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
    expect(await h.view.result.current.pasteFrames()).toBe(false);
  });
  expect(h.apply).not.toHaveBeenCalled();
  expect(h.applyWithOutcome).not.toHaveBeenCalled();
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
