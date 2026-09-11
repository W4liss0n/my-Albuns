import { emptyLayoutCatalogPort, unusedLayoutDialogPort } from "../test/layoutCatalogPorts";
import { useState } from "react";
import { act, renderHook } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { representativeProjection } from "../test/projectFixtures";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

afterEach(() => useEditorView.setState(useEditorView.getInitialState(), true));

test.each([false, true])("ordering followed by Save and Undo uses the queue when failure=%s", async (fails) => {
  const initial = structuredClone(representativeProjection);
  let resolve!: (projection: EditorProjection) => void;
  let reject!: (error: Error) => void;
  const pending = new Promise<EditorProjection>((yes, no) => { resolve = yes; reject = no; });
  const arranged = structuredClone(initial);
  arranged.state.revision += 1;
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this ordering test."); };
  const apply = vi.fn<ProjectCorePort["apply"]>(() => pending);
  const save = vi.fn<ProjectCorePort["save"]>(async (revision) => ({
    outcome: { kind: "saved", revision }, projection: arranged,
  }));
  const undo = vi.fn(async () => initial);
  const port: ProjectCorePort = {
    load: async () => initial, apply, save, undo,
    ...emptyLayoutCatalogPort,
    readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewDecorativeDrop: async () => { throw new Error("Decorative preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: unsupported, redo: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, applyWithOutcome: unsupported,
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
  act(() => {
    completion = Promise.all([view.result.current.arrangeFrames("advance"),
      view.result.current.save(), view.result.current.undo()]);
  });
  expect(save).not.toHaveBeenCalled();
  expect(undo).not.toHaveBeenCalled();
  await act(async () => {
    if (fails) reject(new Error("A ordenação falhou."));
    else resolve(arranged);
    await completion;
  });
  expect(apply.mock.calls[0][0]).toEqual({ kind: "arrangeFrames", frameIds: ["frame-001"], action: "advance" });
  expect(useEditorView.getState().selectedFrameIds).toEqual(["frame-001"]);
  if (fails) {
    expect(save).not.toHaveBeenCalled();
    expect(undo).not.toHaveBeenCalled();
    expect(view.result.current.message).toBe("A ordenação falhou.");
  } else {
    expect(save).toHaveBeenCalledWith(arranged.state.revision);
    expect(undo).toHaveBeenCalledOnce();
  }
});
