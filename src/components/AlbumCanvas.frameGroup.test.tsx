import { act, fireEvent, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { ComposedFrame, FrameGeometryEdit } from "../domain/project";
import { interactiveComposition } from "./albumCanvasTestFixtures";
import {
  displayWithLabel, finishPixiInitialization, getPixiLifecycle, renderCanvas, setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();

function groupComposition() {
  const composition = structuredClone(interactiveComposition);
  const original = composition.sheets[0].frames[0];
  composition.sheets[0].frames = [
    { ...original, photo: null, clipRect: { x: 20_000, y: 20_000, width: 120_000, height: 80_000 } },
    { ...original, frameId: "frame-002", zIndex: 1, photo: null,
      clipRect: { x: 200_000, y: 80_000, width: 60_000, height: 120_000 } },
  ];
  return composition;
}

function press(label: string) {
  const app = getPixiLifecycle().instances[0];
  vi.spyOn(app.canvas, "getBoundingClientRect").mockReturnValue({
    x: 0, y: 0, left: 0, top: 0, right: app.screen.width, bottom: app.screen.height,
    width: app.screen.width, height: app.screen.height, toJSON: () => ({}),
  });
  app.canvas.setPointerCapture = vi.fn();
  app.canvas.releasePointerCapture = vi.fn();
  const target = displayWithLabel(label);
  act(() => target.emit("pointerdown", {
    button: 0, pointerId: 7, clientX: 100, clientY: 100, altKey: false, shiftKey: false,
    stopPropagation: vi.fn(), currentTarget: target,
  }));
}

function latest(label: string) {
  return getPixiLifecycle().displays.filter((item) => item.label === label).slice(-1)[0]!;
}

test.each(["move", "resize"])("%s previews and commits the whole selection without collapsing it", async (action) => {
  const composition = groupComposition();
  const frames = composition.sheets[0].frames;
  const proposed = frames.map((frame) => ({ ...frame, clipRect: { ...frame.clipRect, x: frame.clipRect.x + 40_000 } }));
  let finish!: (frames: ComposedFrame[]) => void;
  const pending = new Promise<ComposedFrame[]>((resolve) => { finish = resolve; });
  const frameGeometry = {
    disabled: false, dragThreshold: { x: 5, y: 5 },
    preview: vi.fn(async (_edit: FrameGeometryEdit) => proposed),
    commit: vi.fn((_edit: FrameGeometryEdit) => pending), onError: vi.fn(),
  };
  const view = renderCanvas({ compositionPlan: composition, mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    selectedFrameIds: ["frame-002", "frame-001"], frameGeometry });
  await finishPixiInitialization();
  press(action === "move" ? "canvas-frame-frame-002" : "frame-resize-handle-bottom-right-group-sheet-001");
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 104, clientY: 103 });
  expect(frameGeometry.preview).not.toHaveBeenCalled();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
  await waitFor(() => expect(latest("canvas-frame-frame-001").position.x).toBe(60));
  expect(latest("canvas-frame-frame-002").position.x).toBe(240);
  expect(latest("frame-selection-container-group-sheet-001").position.x).toBe(60);
  expect(view.onSelectFrame).not.toHaveBeenCalled();
  expect(frameGeometry.preview.mock.calls[0][0].frames).toEqual([
    { frameId: "frame-001", expectedRect: frames[0].clipRect },
    { frameId: "frame-002", expectedRect: frames[1].clipRect },
  ]);
  fireEvent.keyDown(window, { key: "Shift", shiftKey: true });
  await waitFor(() => expect(frameGeometry.preview).toHaveBeenCalledTimes(2));
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 145, clientY: 125, shiftKey: true, altKey: true });
  expect(frameGeometry.commit).toHaveBeenCalledOnce();
  expect(frameGeometry.commit.mock.calls[0][0].gesture).toMatchObject(action === "move"
    ? { kind: "move" } : { kind: "resize", handle: "bottomRight", preserveAspectRatio: true, fromCenter: true });
  await act(async () => finish(proposed));
  expect(latest("canvas-frame-frame-001").position.x).toBe(60);
  expect(latest("canvas-frame-frame-002").position.x).toBe(240);
  const confirmed = structuredClone(composition);
  confirmed.sheets[0].frames = proposed;
  view.rerenderCanvas({ composition: confirmed });
  expect(latest("canvas-frame-frame-002").position.x).toBe(240);
});

test.each(["Escape", "stale-member"])("%s cancels the entire group and ignores its delayed preview", async (reason) => {
  const composition = groupComposition();
  let finish!: (frames: ComposedFrame[]) => void;
  const pending = new Promise<ComposedFrame[]>((resolve) => { finish = resolve; });
  const frameGeometry = { disabled: false, dragThreshold: { x: 5, y: 5 },
    preview: vi.fn(async () => pending), commit: vi.fn(async () => composition.sheets[0].frames), onError: vi.fn() };
  const view = renderCanvas({ compositionPlan: composition, mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    selectedFrameIds: ["frame-001", "frame-002"], frameGeometry });
  await finishPixiInitialization();
  press("canvas-frame-frame-001");
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
  expect(frameGeometry.preview).toHaveBeenCalledOnce();
  if (reason === "Escape") fireEvent.keyDown(window, { key: "Escape" });
  else {
    const changed = structuredClone(composition);
    changed.sheets[0].frames[1].clipRect.x = 210_000;
    view.rerenderCanvas({ composition: changed });
  }
  await act(async () => finish(composition.sheets[0].frames.map((frame) => ({
    ...frame, clipRect: { ...frame.clipRect, x: frame.clipRect.x + 40_000 },
  }))));
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 145, clientY: 125 });
  expect(frameGeometry.commit).not.toHaveBeenCalled();
  expect(latest("canvas-frame-frame-001").position.x).toBe(20);
  expect(latest("canvas-frame-frame-002").position.x).toBe(reason === "Escape" ? 200 : 210);
});

test.each([[true, 1], [true, 2], [false, 1]] as const)("Ctrl-click requests a toggle only in editing mode (%s, click count %i)", async (editing, detail) => {
  const view = renderCanvas({ compositionPlan: groupComposition(),
    mode: editing ? { kind: "sheet-editing", sheetId: "sheet-001" } : { kind: "normal" } });
  await finishPixiInitialization();
  act(() => displayWithLabel("canvas-frame-frame-002").emit("pointertap", {
    button: 0, ctrlKey: true, altKey: false, detail, stopPropagation: vi.fn(),
  }));
  expect(vi.mocked(view.onSelectFrame).mock.calls).toEqual(editing ? [["frame-002", true]] : [["frame-002"]]);
});

test("multiple selected Frames retain individual outlines and share eight bounding-box handles", async () => {
  const composition = structuredClone(interactiveComposition);
  const original = composition.sheets[0].frames[0];
  const frames: ComposedFrame[] = [
    { ...original, photo: null, clipRect: { x: 20_000, y: 20_000, width: 120_000, height: 80_000 } },
    { ...original, frameId: "frame-002", zIndex: 1, photo: null,
      clipRect: { x: 200_000, y: 80_000, width: 60_000, height: 120_000 } },
  ];
  composition.sheets[0].frames = frames;
  renderCanvas({ compositionPlan: composition, mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    selectedFrameIds: ["frame-001", "frame-002"], frameGeometry: {
      disabled: false, dragThreshold: { x: 5, y: 5 },
      preview: vi.fn(async () => frames), commit: vi.fn(async () => frames), onError: vi.fn(),
    } });
  await finishPixiInitialization();
  expect(displayWithLabel("frame-selection-container-frame-001").visible).toBe(true);
  expect(displayWithLabel("frame-selection-container-frame-002").visible).toBe(true);
  expect(displayWithLabel("frame-selection-container-group-sheet-001").position)
    .toMatchObject({ x: 20, y: 20 });
  expect(displayWithLabel("frame-resize-handle-right-group-sheet-001").position)
    .toMatchObject({ x: 240, y: 90 });
  expect(displayWithLabel("frame-resize-handle-right-frame-001").visible).toBe(false);
  expect(displayWithLabel("frame-resize-handle-right-frame-002").visible).toBe(false);
});
