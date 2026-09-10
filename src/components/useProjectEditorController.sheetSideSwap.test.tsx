import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { sheetSideSwapCorpus as corpus } from "../test/sheetSideSwapPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}

function harness(name = "mixed") {
  const scenario = corpus.cases.find((item) => item.name === name)!;
  const initial = structuredClone(scenario.before ?? corpus.before);
  const swapped = structuredClone(scenario.after);
  const pending = deferred<EditorProjection>();
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this side swap test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>(() => pending.promise);
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision }, projection: { ...swapped, state: { ...swapped.state, dirty: false, savedRevision: revision } } }));
  const undo = vi.fn(async () => ({ ...initial, state: { ...initial.state, canRedo: true } }));
  const port: ProjectCorePort = {
    load: async () => initial, apply, applyWithOutcome: unsupported, save, undo, redo: async () => swapped,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: unsupported, saveAs: unsupported, validateAlbumInformation: unsupported,
    importMedia: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: null,
    focusedSheetId: "sheet-002", centeredSheetId: "sheet-002", selectedFrameIds: ["side-frame-1-0"] });
  const view = renderHook(({ blocked }) => {
    const [projection, setProjection] = useState(initial);
    const runner = useProjectMutationRunner(initial.state.projectId, port);
    return { runner, projection, ...useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port,
      runProjectMutation: runner, onProjectionChange: setProjection, interactionBlocked: blocked }) };
  }, { initialProps: { blocked: false } });
  return { view, initial, swapped, scenario, pending, apply, save, undo };
}

test.each(["success", "failure"])("side swap, Save and Undo share the authoritative queue: %s", async (outcome) => {
  const h = harness();
  let command!: Promise<boolean>;
  act(() => {
    command = h.view.result.current.swapSheetSides(h.scenario.targetSheetId);
    h.view.result.current.save();
    h.view.result.current.undo();
  });
  expect(h.save).not.toHaveBeenCalled();
  expect(h.undo).not.toHaveBeenCalled();
  await act(async () => {
    if (outcome === "success") h.pending.resolve(h.swapped);
    else h.pending.reject(new Error("Falha ao trocar os lados."));
    await command;
    await h.view.result.current.runner.waitForIdle();
  });
  expect(h.apply.mock.calls[0][0]).toEqual({ kind: "swapSheetSides", sheetId: h.scenario.targetSheetId });
  if (outcome === "success") {
    expect(h.save).toHaveBeenCalledWith(h.swapped.state.revision);
    expect(h.undo).toHaveBeenCalledOnce();
  } else {
    expect(h.save).not.toHaveBeenCalled();
    expect(h.undo).not.toHaveBeenCalled();
    expect(h.view.result.current.message).toBe("Falha ao trocar os lados.");
  }
  expect(h.view.result.current.projection.state.album).toEqual(h.initial.state.album);
  expect(useEditorView.getState()).toMatchObject({ focusedSheetId: "sheet-002", centeredSheetId: "sheet-002", selectedFrameIds: ["side-frame-1-0"], editingSheetId: null });
});

test("the explicit Sheet is swapped without navigation, and Redo reapplies it", async () => {
  const h = harness();
  await act(async () => { const command = h.view.result.current.swapSheetSides(h.scenario.targetSheetId); h.pending.resolve(h.swapped); await command; });
  expect(h.view.result.current.projection).toEqual(h.swapped);
  expect(useEditorView.getState()).toMatchObject({ focusedSheetId: "sheet-002", centeredSheetId: "sheet-002", selectedFrameIds: ["side-frame-1-0"] });
  await act(async () => { h.view.result.current.undo(); h.view.result.current.redo(); await h.view.result.current.runner.waitForIdle(); });
  expect(h.view.result.current.projection).toEqual(h.swapped);
});

test.each(["right-page", "left-page"])("side swap is unavailable on %s", async (name) => {
  const h = harness(name);
  await act(async () => expect(await h.view.result.current.swapSheetSides(h.scenario.targetSheetId)).toBe(false));
  expect(h.apply).not.toHaveBeenCalled();
});

test("side swap respects blocked interactions and isolated Sheet editing", async () => {
  const h = harness();
  h.view.rerender({ blocked: true });
  await act(async () => expect(await h.view.result.current.swapSheetSides(h.scenario.targetSheetId)).toBe(false));
  h.view.rerender({ blocked: false });
  act(() => useEditorView.getState().enterSheetEdit(h.scenario.targetSheetId));
  await act(async () => expect(await h.view.result.current.swapSheetSides(h.scenario.targetSheetId)).toBe(false));
  expect(h.apply).not.toHaveBeenCalled();
});

test.each(["deleted", "converted"])("a queued swap is cancelled if its explicit Sheet was %s", async (change) => {
  const h = harness();
  const predecessor = deferred<EditorProjection>();
  const current = structuredClone(h.initial);
  if (change === "deleted") current.state.album.sheets = current.state.album.sheets.filter((sheet) => sheet.id !== h.scenario.targetSheetId);
  else current.state.album.sheets.find((sheet) => sheet.id === h.scenario.targetSheetId)!.activeSides = "right";
  let command!: Promise<boolean>;
  act(() => {
    void h.view.result.current.runner.run(() => predecessor.promise);
    command = h.view.result.current.swapSheetSides(h.scenario.targetSheetId);
  });
  await act(async () => { predecessor.resolve(current); expect(await command).toBe(false); });
  expect(h.apply).not.toHaveBeenCalled();
});
