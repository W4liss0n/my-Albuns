import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { frameDeletionCorpus as corpus } from "../test/frameDeletionPreview";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

function deletionHarness() {
  const initial = structuredClone(corpus.before) as EditorProjection;
  const deletion = corpus.cases.find((item) => item.name === "group")!;
  const deleted = structuredClone(deletion.after) as EditorProjection;
  let resolve!: (projection: EditorProjection) => void;
  let reject!: (error: Error) => void;
  const pending = new Promise<EditorProjection>((yes, no) => { resolve = yes; reject = no; });
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this deletion test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>(() => pending);
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision }, projection: deleted }));
  const undo = vi.fn(async () => initial);
  const redo = vi.fn(async () => deleted);
  const port: ProjectCorePort = {
    load: async () => initial, apply, save, undo, redo,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, applyWithOutcome: unsupported,
    importMedia: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: deletion.selectedFrameIds });
  const view = renderHook(({ blocked }) => {
    const [projection, setProjection] = useState(initial);
    const runProjectMutation = useProjectMutationRunner(initial.state.projectId, port);
    return useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port,
      runProjectMutation, onProjectionChange: setProjection, interactionBlocked: blocked });
  }, { initialProps: { blocked: false } });
  return { view, initial, deleted, resolve, reject, apply, save, undo, redo };
}

test.each([false, true])("deletion followed by Save and Undo respects the queue and selection when failure=%s", async (fails) => {
  const harness = deletionHarness();
  const { view, apply, save, undo, deleted } = harness;
  let completion!: Promise<unknown>;
  act(() => {
    completion = Promise.all([view.result.current.deleteFrames(), view.result.current.save(), view.result.current.undo()]);
    // A later selection must survive; the captured deleted IDs must not return on Undo,
    // including when React presents the delete/save/undo results together.
    useEditorView.getState().selectFrame("delete-frame-3", true);
  });
  expect(save).not.toHaveBeenCalled();
  expect(undo).not.toHaveBeenCalled();
  await act(async () => {
    if (fails) harness.reject(new Error("A exclusão falhou."));
    else harness.resolve(deleted);
    await completion;
  });
  expect(apply.mock.calls[0][0]).toEqual({ kind: "deleteFrames", frameIds: ["delete-frame-1", "delete-frame-2"], mode: "edit" });
  if (fails) {
    expect(save).not.toHaveBeenCalled();
    expect(undo).not.toHaveBeenCalled();
    expect(view.result.current.message).toBe("A exclusão falhou.");
    expect(useEditorView.getState().selectedFrameIds).toEqual(["delete-frame-1", "delete-frame-2", "delete-frame-3"]);
  } else {
    expect(save).toHaveBeenCalledWith(deleted.state.revision);
    expect(undo).toHaveBeenCalledOnce();
    expect(useEditorView.getState().selectedFrameIds).toEqual(["delete-frame-3"]);
    expect(view.result.current.selectedFrames.map((frame) => frame.id)).toEqual(["delete-frame-3"]);
  }
});

test("deletion clears removed selection and Undo/Redo do not select restored Frames", async () => {
  const { view, resolve, deleted } = deletionHarness();
  await act(async () => {
    const completion = view.result.current.deleteFrames();
    resolve(deleted);
    await completion;
  });
  expect(view.result.current.canDeleteFrames).toBe(false);
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
  await act(async () => { await view.result.current.undo(); });
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
  await act(async () => { await view.result.current.redo(); });
  expect(useEditorView.getState().selectedFrameIds).toEqual([]);
});

test("deletion rejects blocked or empty selection and works in normal mode", async () => {
  const { view, apply, resolve, deleted } = deletionHarness();
  view.rerender({ blocked: true });
  await act(async () => { expect(await view.result.current.deleteFrames()).toBe(false); });
  view.rerender({ blocked: false });
  act(() => useEditorView.getState().selectFrame(null));
  await act(async () => { expect(await view.result.current.deleteFrames()).toBe(false); });
  act(() => {
    useEditorView.getState().exitSheetEdit();
    useEditorView.getState().selectFrame("delete-frame-0");
  });
  expect(apply).not.toHaveBeenCalled();
  await act(async () => {
    const completion = view.result.current.deleteFrames();
    resolve(deleted);
    expect(await completion).toBe(true);
  });
  expect(apply.mock.calls[0][0]).toEqual({ kind: "deleteFrames", frameIds: ["delete-frame-0"], mode: "normal" });
});
