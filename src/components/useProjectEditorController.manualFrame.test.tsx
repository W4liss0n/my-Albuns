import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import { useEditorView } from "../state/editorView";
import { representativeProjection } from "../test/projectFixtures";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

test.each([false, true])("creation followed by Save and Undo respects the queue when failure=%s", async (fails) => {
  const initial = structuredClone(representativeProjection);
  const created = structuredClone(initial);
  created.state.revision += 1;
  created.state.canUndo = true;
  created.state.album.sheets[0].frames.push({
    ...created.state.album.sheets[0].frames[0], id: "manual-frame", zIndex: 1, photo: null,
  });
  created.composition.sheets[0].frames.push({
    ...created.composition.sheets[0].frames[0], frameId: "manual-frame", zIndex: 1, photo: null,
  });
  let resolve!: (result: Awaited<ReturnType<ProjectCorePort["applyWithOutcome"]>>) => void;
  let reject!: (error: Error) => void;
  const pending = new Promise<Awaited<ReturnType<ProjectCorePort["applyWithOutcome"]>>>((yes, no) => { resolve = yes; reject = no; });
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this creation test."); };
  const applyWithOutcome = vi.fn<ProjectCorePort["applyWithOutcome"]>(() => pending);
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({ outcome: { kind: "saved", revision }, projection: created }));
  const undo = vi.fn(async () => initial);
  const port: ProjectCorePort = {
    load: async () => initial, applyWithOutcome, save, undo,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: unsupported, redo: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, apply: unsupported,
    importMedia: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  useEditorView.setState({ projectId: initial.state.projectId, editingSheetId: "sheet-001",
    focusedSheetId: "sheet-001", centeredSheetId: "sheet-001", selectedFrameIds: ["frame-001"] });
  const view = renderHook(() => {
    const [projection, setProjection] = useState(initial);
    const runProjectMutation = useProjectMutationRunner(initial.state.projectId, port);
    return useProjectEditorController({ projectDialogPort: unusedLayoutDialogPort, projection, projectCorePort: port,
      runProjectMutation, onProjectionChange: setProjection });
  });
  let completion!: Promise<unknown>;
  act(() => { completion = Promise.all([view.result.current.addFrame(), view.result.current.save(), view.result.current.undo()]); });
  expect(save).not.toHaveBeenCalled();
  expect(undo).not.toHaveBeenCalled();
  await act(async () => {
    if (fails) reject(new Error("A criação falhou."));
    else resolve({ projection: created, affectedFrameId: "manual-frame", affectedSheetId: null });
    await completion;
  });
  expect(applyWithOutcome.mock.calls[0][0]).toEqual({ kind: "addFrame", sheetId: "sheet-001" });
  if (fails) {
    expect(save).not.toHaveBeenCalled();
    expect(undo).not.toHaveBeenCalled();
    expect(view.result.current.message).toBe("A criação falhou.");
    expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001"]);
  } else {
    expect(save).toHaveBeenCalledWith(created.state.revision);
    expect(undo).toHaveBeenCalledOnce();
    expect(useEditorView.getState().selectedFrameIds).toEqual([]);
  }
});
