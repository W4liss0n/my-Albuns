import { useLayoutEffect, useState } from "react";
import { act, fireEvent, render, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { ProjectCorePort } from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import { representativeProjection } from "../test/projectFixtures";
import {
  AlbumCanvas, finishPixiInitialization, getPixiLifecycle, setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";
import { useProjectEditorController } from "./useProjectEditorController";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

setupAlbumCanvasTestHarness();

test.each([1, 2])("releasing %i Frames and immediately undoing presents the latest history and permits another gesture", async (count) => {
  const initial = structuredClone(representativeProjection);
  initial.composition.sheets[0].frames[0].photo = null;
  initial.state.album.sheets[0].frames[0].photo = null;
  if (count === 2) {
    initial.composition.sheets[0].frames.push({ ...initial.composition.sheets[0].frames[0], frameId: "frame-002",
      clipRect: { x: 200_000, y: 60_000, width: 80_000, height: 100_000 } });
    initial.state.album.sheets[0].frames.push({ ...initial.state.album.sheets[0].frames[0], id: "frame-002",
      rect: { x: 200_000, y: 60_000, width: 80_000, height: 100_000 } });
  }
  const changed = structuredClone(initial);
  changed.state.revision += 1;
  for (const frame of changed.composition.sheets[0].frames) frame.clipRect.x += 40_000;
  for (const frame of changed.state.album.sheets[0].frames) frame.rect.x += 40_000;
  const undone = structuredClone(initial);
  // The Core restores the historical revision number on Undo.
  undone.state.canRedo = true;
  let resolveEdit!: (projection: EditorProjection) => void;
  const pendingEdit = new Promise<EditorProjection>((resolve) => { resolveEdit = resolve; });
  const unsupported = async (): Promise<never> => { throw new Error("Unsupported in this Frame/history test."); };
  const preview = vi.fn(async () => changed.composition.sheets[0].frames);
  const undo = vi.fn(async () => undone);
  const port: ProjectCorePort = {
    load: async () => initial, apply: async () => pendingEdit,
    readSliderDoubleClickTime: async () => 500,
    queryLayouts: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewLayout: async () => { throw new Error("Layouts are not configured in this fixture."); },
    previewFrameStyle: async () => { throw new Error("Frame style preview is not configured in this fixture."); },
    previewPhotoAngle: async () => { throw new Error("Photo angle preview is not configured in this fixture."); },
    previewFrameGeometry: preview, readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
    undo, redo: unsupported, save: unsupported, saveAs: unsupported,
    validateAlbumInformation: unsupported, applyWithOutcome: unsupported,
    importPhoto: unsupported, resolvePhotoDropTarget: unsupported, relink: unsupported,
  };
  useEditorView.setState({
    projectId: initial.state.projectId, editingSheetId: "sheet-001",
    selectedFrameIds: initial.state.album.sheets[0].frames.map((frame) => frame.id), focusedSheetId: "sheet-001", centeredSheetId: "sheet-001",
    viewport: { offsetX: 0 },
  });
  const presentedRevisions: number[] = [];
  function Editor() {
    const [projection, setProjection] = useState(initial);
    const runProjectMutation = useProjectMutationRunner(initial.state.projectId, port);
    const controller = useProjectEditorController({
      projection, projectCorePort: port, runProjectMutation, onProjectionChange: setProjection,
    });
    useLayoutEffect(() => { presentedRevisions.push(projection.state.revision); }, [projection]);
    return <>
      <button onClick={controller.undo}>Desfazer</button>
      <AlbumCanvas {...controller.canvasProps} />
    </>;
  }
  const view = render(<Editor />);
  await finishPixiInitialization();
  const app = getPixiLifecycle().instances[0];
  vi.spyOn(app.canvas, "getBoundingClientRect").mockReturnValue({
    x: 0, y: 0, left: 0, top: 0, right: app.screen.width, bottom: app.screen.height,
    width: app.screen.width, height: app.screen.height, toJSON: () => ({}),
  });
  app.canvas.setPointerCapture = vi.fn();
  app.canvas.releasePointerCapture = vi.fn();
  const frame = () => getPixiLifecycle().displays
    .filter((item) => item.label === "canvas-frame-frame-001").slice(-1)[0]!;
  const press = () => {
    const target = frame();
    act(() => target.emit("pointerdown", {
      button: 0, pointerId: 7, clientX: 100, clientY: 100,
      altKey: false, shiftKey: false, stopPropagation: vi.fn(), currentTarget: target,
    }));
  };
  press();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 100 });
  await waitFor(() => expect(frame().position.x).toBe(60));
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 140, clientY: 100 });
  fireEvent.click(view.getByRole("button", { name: "Desfazer" }));
  expect(undo).not.toHaveBeenCalled();
  // Both results flow through the production queue and React state in one batch.
  await act(async () => { resolveEdit(changed); });
  expect(undo).toHaveBeenCalledOnce();
  expect(presentedRevisions).toEqual([initial.state.revision, undone.state.revision]);
  expect(frame().position.x).toBe(20);
  expect(useEditorView.getState().selectedFrameIds).toEqual(count === 2 ? ["frame-001", "frame-002"] : ["frame-001"]);
  if (count === 2) expect(getPixiLifecycle().displays
    .filter((item) => item.label === "canvas-frame-frame-002").slice(-1)[0]!.position.x).toBe(200);
  preview.mockClear();
  press();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 145, clientY: 100 });
  await waitFor(() => expect(preview).toHaveBeenCalledOnce());
});
