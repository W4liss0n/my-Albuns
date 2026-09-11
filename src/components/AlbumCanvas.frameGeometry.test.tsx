import { act, fireEvent, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { frameGeometryPreview } from "../test/frameGeometryPreview";

import type { ComposedFrame, FrameGeometryEdit, FrameResizeHandle } from "../domain/project";
import { interactiveComposition } from "./albumCanvasTestFixtures";
import {
  displayWithLabel, finishPixiInitialization, getPixiLifecycle,
  renderCanvas, setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();

function preparePointerCanvas() {
  const app = getPixiLifecycle().instances[0];
  vi.spyOn(app.canvas, "getBoundingClientRect").mockReturnValue({
    x: 0, y: 0, left: 0, top: 0, right: app.screen.width, bottom: app.screen.height,
    width: app.screen.width, height: app.screen.height, toJSON: () => ({}),
  });
  app.canvas.setPointerCapture = vi.fn();
  app.canvas.releasePointerCapture = vi.fn();
  return app;
}

function startPointer(label = "canvas-frame-frame-001") {
  const target = displayWithLabel(label);
  act(() => target.emit("pointerdown", {
    button: 0, pointerId: 7, clientX: 100, clientY: 100,
    altKey: false, shiftKey: false, stopPropagation: vi.fn(), currentTarget: target,
  }));
}

function latestFrame() {
  return getPixiLifecycle().displays.filter((item) => item.label === "canvas-frame-frame-001").slice(-1)[0]!;
}

function controls() {
  return {
    disabled: false, dragThreshold: { x: 5, y: 5 },
    preview: vi.fn(async (_edit: FrameGeometryEdit) => frameGeometryPreview([interactiveComposition.sheets[0].frames[0]])),
    commit: vi.fn(async (_edit: FrameGeometryEdit): Promise<ComposedFrame[] | null> => [interactiveComposition.sheets[0].frames[0]]), onError: vi.fn(),
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((complete) => { resolve = complete; });
  return { promise, resolve };
}

test("Control removes snap guides while preview is pending and release reacquires without retention", async () => {
  const frameGeometry = controls();
  const snapped = { ...frameGeometryPreview([interactiveComposition.sheets[0].frames[0]]), snap: {
    retained: ["alignment-x"], guides: [{ kind: "alignment" as const, axis: "x" as const,
      x1: 150_000, y1: 0, x2: 150_000, y2: 300_000, measurementUm: null }],
  } };
  frameGeometry.preview.mockResolvedValue(snapped);
  renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, compositionPlan: interactiveComposition, selectedFrameId: "frame-001", frameGeometry });
  await finishPixiInitialization();
  preparePointerCanvas(); startPointer();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
  const guideLayer = () => getPixiLifecycle().displays.filter((item) => item.label === "frame-snap-guides").slice(-1)[0]!;
  await waitFor(() => expect(guideLayer().visible).toBe(true));
  const free = deferred<ReturnType<typeof frameGeometryPreview>>();
  frameGeometry.preview.mockReturnValueOnce(free.promise);
  fireEvent.keyDown(window, { key: "Control", ctrlKey: true });
  expect(guideLayer().visible).toBe(false);
  expect(frameGeometry.preview.mock.calls.slice(-1)[0]![0].snap).toBeUndefined();
  fireEvent.keyUp(window, { key: "Control", ctrlKey: false });
  await act(async () => free.resolve(frameGeometryPreview([interactiveComposition.sheets[0].frames[0]])));
  await waitFor(() => expect(frameGeometry.preview).toHaveBeenCalledTimes(3));
  expect(frameGeometry.preview.mock.calls.slice(-1)[0]![0].snap?.retained).toEqual([]);
  await waitFor(() => expect(guideLayer().visible).toBe(true));
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 140, clientY: 120 });
  expect(guideLayer().visible).toBe(false);
});

test("a snap preview resolved after Control was pressed cannot restore its geometry or guides", async () => {
  const frameGeometry = controls();
  const stale = deferred<ReturnType<typeof frameGeometryPreview>>();
  const free = deferred<ReturnType<typeof frameGeometryPreview>>();
  frameGeometry.preview.mockReturnValueOnce(stale.promise).mockReturnValueOnce(free.promise);
  renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, compositionPlan: interactiveComposition, selectedFrameId: "frame-001", frameGeometry });
  await finishPixiInitialization();
  preparePointerCanvas(); startPointer();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
  fireEvent.keyDown(window, { key: "Control", ctrlKey: true });
  await act(async () => stale.resolve({
    frames: [{ ...interactiveComposition.sheets[0].frames[0], clipRect: { x: 150_000, y: 0, width: 200_000, height: 200_000 } }],
    snap: { retained: ["old"], guides: [{ kind: "alignment", axis: "x", x1: 150_000, y1: 0, x2: 150_000, y2: 300_000, measurementUm: null }] },
  }));
  expect(latestFrame().position.x).toBe(0);
  expect(getPixiLifecycle().displays.filter((item) => item.label === "frame-snap-guides").slice(-1)[0]!.visible).toBe(false);
  await act(async () => free.resolve(frameGeometryPreview([interactiveComposition.sheets[0].frames[0]])));
});

test("dragging a locked selection reports the lock after the threshold without requesting geometry", async () => {
  const frameGeometry = controls();
  renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, selectedFrameId: "frame-001", frameGeometry,
    compositionPlan: interactiveComposition,
    sheetBarMetadata: [{ sheetId: "sheet-001", pageNumbers: [1, 2], layoutLocked: true }] });
  await finishPixiInitialization();
  preparePointerCanvas();
  startPointer();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 102, clientY: 100 });
  expect(frameGeometry.onError).not.toHaveBeenCalled();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 150, clientY: 120 });
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 150, clientY: 120 });
  expect(frameGeometry.onError).toHaveBeenCalledExactlyOnceWith("O Layout está travado. Destrave-o no Painel de Layouts para mover os Frames.");
  expect(frameGeometry.preview).not.toHaveBeenCalled();
  expect(frameGeometry.commit).not.toHaveBeenCalled();
});

test.each(["move", "resize"])(
  "%s does not flash the old Frame between commit completion and the confirmed projection", async (action) => {
    const composition = structuredClone(interactiveComposition);
    const proposed: ComposedFrame = {
      ...composition.sheets[0].frames[0],
      clipRect: { x: 40_000, y: 20_000, width: 250_000, height: 150_000 },
    };
    const commit = deferred<ComposedFrame[] | null>();
    const frameGeometry = controls();
    frameGeometry.preview.mockResolvedValue(frameGeometryPreview([proposed]));
    frameGeometry.commit.mockReturnValue(commit.promise);
    const view = renderCanvas({
      mode: { kind: "sheet-editing", sheetId: "sheet-001" },
      compositionPlan: composition, selectedFrameId: "frame-001", frameGeometry,
    });
    await finishPixiInitialization();
    preparePointerCanvas();
    startPointer(action === "resize" ? "frame-resize-handle-top-left-frame-001" : undefined);
    fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
    await waitFor(() => expect(latestFrame().position).toMatchObject({ x: 40, y: 20 }));
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 140, clientY: 120 });
    await act(async () => commit.resolve([proposed]));
    // React may present the authoritative projection after the command promise resolves.
    expect(latestFrame().position).toMatchObject({ x: 40, y: 20 });
    composition.sheets[0].frames[0] = proposed;
    view.rerenderCanvas({ composition });
    expect(latestFrame().position).toMatchObject({ x: 40, y: 20 });
  },
);

test("presents the exact committed Frame when release differs from the last preview", async () => {
  const composition = structuredClone(interactiveComposition);
  const original = composition.sheets[0].frames[0];
  const preview = { ...original, clipRect: { ...original.clipRect, x: 40_000, y: 20_000 } };
  const committed = { ...original, clipRect: { ...original.clipRect, x: 55_000, y: 25_000 } };
  const reply = deferred<ComposedFrame[] | null>();
  const frameGeometry = controls();
  frameGeometry.preview.mockResolvedValue(frameGeometryPreview([preview]));
  frameGeometry.commit.mockReturnValue(reply.promise);
  const view = renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    compositionPlan: composition, frameGeometry });
  await finishPixiInitialization();
  preparePointerCanvas();
  startPointer();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
  await waitFor(() => expect(latestFrame().position.x).toBe(40));
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 155, clientY: 125 });
  await act(async () => reply.resolve([committed]));
  expect(latestFrame().position).toMatchObject({ x: 55, y: 25 });
  composition.sheets[0].frames[0] = committed;
  view.rerenderCanvas({ composition });
  expect(latestFrame().position).toMatchObject({ x: 55, y: 25 });
});

test("a no-op receipt restores the original Frame and allows the next gesture without a new projection", async () => {
  const original = interactiveComposition.sheets[0].frames[0];
  const frameGeometry = controls();
  frameGeometry.preview.mockResolvedValue(frameGeometryPreview([{ ...original, clipRect: { ...original.clipRect, x: 40_000 } }]));
  frameGeometry.commit.mockResolvedValue([original]);
  renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    compositionPlan: interactiveComposition, frameGeometry });
  await finishPixiInitialization();
  preparePointerCanvas();
  startPointer();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 100 });
  await waitFor(() => expect(latestFrame().position.x).toBe(40));
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 100, clientY: 100 });
  await waitFor(() => expect(latestFrame().position.x).toBe(0));
  const target = latestFrame();
  act(() => target.emit("pointerdown", { button: 0, pointerId: 9, clientX: 100, clientY: 100,
    altKey: false, shiftKey: false, currentTarget: target, stopPropagation: vi.fn() }));
  fireEvent.pointerMove(window, { pointerId: 9, clientX: 160, clientY: 110 });
  await waitFor(() => expect(frameGeometry.preview).toHaveBeenCalledTimes(2));
});

test("previews a Frame drag only beyond the platform threshold and commits the released position once", async () => {
  const original = interactiveComposition.sheets[0].frames[0];
  const proposed: ComposedFrame = { ...original, clipRect: { ...original.clipRect, x: 40_000, y: 20_000 } };
  const reply = deferred<ComposedFrame[]>();
  const commit = deferred<ComposedFrame[] | null>();
  const frameGeometry = {
    disabled: false, dragThreshold: { x: 5, y: 5 },
    preview: vi.fn((_edit: FrameGeometryEdit) => reply.promise.then(frameGeometryPreview)),
    commit: vi.fn((_edit: FrameGeometryEdit) => commit.promise), onError: vi.fn(),
  };
  const view = renderCanvas({
    mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    compositionPlan: interactiveComposition, frameGeometry,
  });
  await finishPixiInitialization();
  const app = preparePointerCanvas();
  startPointer();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 103, clientY: 102 });
  expect(frameGeometry.preview).not.toHaveBeenCalled();
  expect(frameGeometry.commit).not.toHaveBeenCalled();

  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
  await waitFor(() => expect(frameGeometry.preview).toHaveBeenCalledOnce());
  expect(view.onSelectFrame).toHaveBeenLastCalledWith("frame-001");
  await act(async () => reply.resolve([proposed]));
  expect(frameGeometry.commit).not.toHaveBeenCalled();
  expect(latestFrame().position).toMatchObject({ x: 40, y: 20 });

  fireEvent.pointerUp(window, { pointerId: 7, clientX: 145, clientY: 123 });
  expect(app.canvas.style.getPropertyValue("--frame-gesture-cursor")).toBe("");
  expect(app.canvas).not.toHaveClass("pixi-canvas--frame-gesture");
  vi.mocked(view.onSelectFrame).mockClear();
  act(() => latestFrame().emit("pointertap", {
    button: 0, altKey: false, stopPropagation: vi.fn(), nativeEvent: { detail: 1 },
  }));
  expect(view.onSelectFrame).not.toHaveBeenCalled();
  expect(frameGeometry.commit).toHaveBeenCalledOnce();
  const edit = frameGeometry.commit.mock.calls[0][0];
  expect(edit).toMatchObject({ frames: [{ frameId: "frame-001", expectedRect: original.clipRect }], gesture: { kind: "move" } });
  expect(edit.gesture).not.toEqual(frameGeometry.preview.mock.calls[0][0].gesture);
  expect(app.canvas.releasePointerCapture).toHaveBeenCalledWith(7);
  await act(async () => commit.resolve(null));
  expect(latestFrame().position).toMatchObject({ x: 0, y: 0 });
});

test.each<[string, FrameResizeHandle]>([
  ["top-left", "topLeft"], ["top", "top"], ["top-right", "topRight"], ["right", "right"],
  ["bottom-right", "bottomRight"], ["bottom", "bottom"], ["bottom-left", "bottomLeft"], ["left", "left"],
])("routes the %s handle through the Core and reads Shift/Alt again on release", async (label, handle) => {
  const frameGeometry = controls();
  renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, compositionPlan: interactiveComposition,
    selectedFrameId: "frame-001", frameGeometry });
  await finishPixiInitialization();
  const app = preparePointerCanvas();
  startPointer(`frame-resize-handle-${label}-frame-001`);
  expect(app.canvas.style.getPropertyValue("--frame-gesture-cursor"))
    .toBe(displayWithLabel(`frame-resize-handle-${label}-frame-001`).cursor);
  expect(app.canvas).toHaveClass("pixi-canvas--frame-gesture");
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 130, clientY: 120 });
  await waitFor(() => expect(frameGeometry.preview).toHaveBeenCalledOnce());
  expect(frameGeometry.preview.mock.calls[0][0].gesture).toMatchObject({
    kind: "resize", handle, preserveAspectRatio: false, fromCenter: false,
  });
  fireEvent.keyDown(window, { key: "Shift", shiftKey: true });
  await waitFor(() => expect(frameGeometry.preview).toHaveBeenCalledTimes(2));
  expect(frameGeometry.preview.mock.calls[1][0].gesture).toMatchObject({ preserveAspectRatio: true });
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 130, clientY: 120, shiftKey: true, altKey: true });
  expect(app.canvas).not.toHaveClass("pixi-canvas--frame-gesture");
  await waitFor(() => expect(frameGeometry.commit).toHaveBeenCalledOnce());
  expect(frameGeometry.commit.mock.calls[0][0]).toMatchObject({
    frames: [{ expectedRect: interactiveComposition.sheets[0].frames[0].clipRect }],
    gesture: { kind: "resize", handle, preserveAspectRatio: true, fromCenter: true },
  });
});

test.each(["Escape", "pointercancel", "blur", "lostpointercapture", "save-shortcut", "context-loss"])(
  "%s cancels an unfinished drag and ignores its delayed preview", async (cancellation) => {
    const reply = deferred<ComposedFrame[]>();
    const frameGeometry = controls();
    frameGeometry.preview.mockReturnValue(reply.promise.then(frameGeometryPreview));
    const view = renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" },
      compositionPlan: interactiveComposition, frameGeometry });
    await finishPixiInitialization();
    const app = preparePointerCanvas();
    startPointer();
    fireEvent.pointerMove(window, { pointerId: 7, clientX: 130, clientY: 120 });
    expect(frameGeometry.preview).toHaveBeenCalledOnce();
    const laterShortcut = vi.fn();
    window.addEventListener("keydown", laterShortcut, { once: true });
    if (cancellation === "Escape") fireEvent.keyDown(window, { key: "Escape" });
    if (cancellation === "pointercancel") fireEvent.pointerCancel(window, { pointerId: 7 });
    if (cancellation === "blur") fireEvent.blur(window);
    if (cancellation === "lostpointercapture") fireEvent(app.canvas, new PointerEvent("lostpointercapture", { pointerId: 7 }));
    if (cancellation === "save-shortcut") fireEvent.keyDown(window, { key: "s", ctrlKey: true });
    if (cancellation === "context-loss") fireEvent(app.canvas, new Event("webglcontextlost", { cancelable: true }));
    expect(app.canvas.style.getPropertyValue("--frame-gesture-cursor")).toBe("");
    expect(app.canvas).not.toHaveClass("pixi-canvas--frame-gesture");
    if (cancellation === "Escape") expect(laterShortcut).not.toHaveBeenCalled();
    if (cancellation === "save-shortcut") expect(laterShortcut).toHaveBeenCalledOnce();
    window.removeEventListener("keydown", laterShortcut);
    await act(async () => reply.resolve([{ ...interactiveComposition.sheets[0].frames[0],
      clipRect: { x: 40_000, y: 20_000, width: 300_000, height: 200_000 } }]));
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 140, clientY: 125 });
    expect(frameGeometry.commit).not.toHaveBeenCalled();
    expect(frameGeometry.onError).not.toHaveBeenCalled();
    expect(latestFrame().position).toMatchObject({ x: 0, y: 0 });
    expect(view.onEditSheet).not.toHaveBeenCalled();
  },
);

test("coalesces slow previews and queues the released position before a pending preview completes", async () => {
  const first = deferred<ComposedFrame[]>();
  const last = deferred<ComposedFrame[]>();
  const frameGeometry = controls();
  frameGeometry.preview.mockReturnValueOnce(first.promise.then(frameGeometryPreview)).mockReturnValueOnce(last.promise.then(frameGeometryPreview));
  renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, compositionPlan: interactiveComposition, frameGeometry });
  await finishPixiInitialization();
  preparePointerCanvas();
  startPointer();
  for (const clientX of [110, 120, 130, 140]) fireEvent.pointerMove(window, { pointerId: 7, clientX, clientY: 110 });
  expect(frameGeometry.preview).toHaveBeenCalledOnce();
  await act(async () => first.resolve([interactiveComposition.sheets[0].frames[0]]));
  expect(frameGeometry.preview).toHaveBeenCalledTimes(2);
  expect(frameGeometry.preview.mock.calls[1][0].gesture).toMatchObject({ deltaXUm: Math.round(40 / 1.48 / 0.001) });
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 150, clientY: 115 });
  expect(frameGeometry.commit).toHaveBeenCalledOnce();
  expect(frameGeometry.commit.mock.calls[0][0].gesture).toMatchObject({ deltaXUm: Math.round(50 / 1.48 / 0.001) });
  await act(async () => last.resolve([{ ...interactiveComposition.sheets[0].frames[0],
    clipRect: { x: 40_000, y: 20_000, width: 300_000, height: 200_000 } }]));
  expect(latestFrame().position).toMatchObject({ x: 0, y: 0 });
});

test("a click below threshold selects without editing and an unrelated pointer cannot end the gesture", async () => {
  const frameGeometry = controls();
  const view = renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, compositionPlan: interactiveComposition, frameGeometry });
  await finishPixiInitialization();
  preparePointerCanvas();
  startPointer();
  fireEvent.pointerMove(window, { pointerId: 8, clientX: 500, clientY: 400 });
  fireEvent.pointerUp(window, { pointerId: 8, clientX: 500, clientY: 400 });
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 105, clientY: 105 });
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 105, clientY: 105 });
  act(() => displayWithLabel("canvas-frame-frame-001").emit("pointertap", {
    button: 0, altKey: false, stopPropagation: vi.fn(), nativeEvent: { detail: 1 },
  }));
  expect(view.onSelectFrame).toHaveBeenCalledWith("frame-001");
  expect(frameGeometry.preview).not.toHaveBeenCalled();
  expect(frameGeometry.commit).not.toHaveBeenCalled();
});

test.each(["project", "mode", "blocking-operation", "confirmed-geometry"])(
  "changing %s discards an unfinished gesture and ignores the old response", async (change) => {
    const frameGeometry = controls();
    const reply = deferred<ComposedFrame[]>();
    frameGeometry.preview.mockReturnValue(reply.promise.then(frameGeometryPreview));
    const view = renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, compositionPlan: interactiveComposition, frameGeometry });
    await finishPixiInitialization();
    preparePointerCanvas();
    startPointer();
    fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 130 });
    expect(frameGeometry.preview).toHaveBeenCalledOnce();
    if (change === "project") view.rerenderCanvas({ projectId: "another-project" });
    if (change === "mode") view.rerenderCanvas({ mode: { kind: "normal" } });
    if (change === "blocking-operation") view.rerenderCanvas({ frameGeometry: { ...frameGeometry, disabled: true } });
    if (change === "confirmed-geometry") {
      const composition = structuredClone(interactiveComposition);
      composition.sheets[0].frames[0].clipRect.x = 10_000;
      view.rerenderCanvas({ composition });
    }
    await act(async () => reply.resolve([{ ...interactiveComposition.sheets[0].frames[0],
      clipRect: { x: 40_000, y: 20_000, width: 300_000, height: 200_000 } }]));
    fireEvent.pointerUp(window, { pointerId: 7, clientX: 160, clientY: 140 });
    expect(frameGeometry.commit).not.toHaveBeenCalled();
    expect(frameGeometry.onError).not.toHaveBeenCalled();
    expect(latestFrame().position.x).toBe(change === "confirmed-geometry" ? 10 : 0);
    // A cancelled drag must not keep normal Canvas selection disabled.
    view.rerenderCanvas({ mode: { kind: "normal" } });
    vi.mocked(view.onSelectFrame).mockClear();
    await waitFor(() => {
      act(() => latestFrame().emit("pointertap", {
        button: 0, altKey: false, stopPropagation: vi.fn(), nativeEvent: { detail: 1 },
      }));
      expect(view.onSelectFrame).toHaveBeenCalledWith("frame-001");
    });
  },
);

test("a failed preview restores the confirmed Frame and reports one error", async () => {
  const frameGeometry = controls();
  frameGeometry.preview.mockRejectedValue(new Error("A geometria do Frame mudou."));
  renderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" }, compositionPlan: interactiveComposition, frameGeometry });
  await finishPixiInitialization();
  preparePointerCanvas();
  startPointer();
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 130 });
  await waitFor(() => expect(frameGeometry.onError).toHaveBeenCalledWith("A geometria do Frame mudou."));
  fireEvent.pointerUp(window, { pointerId: 7, clientX: 150, clientY: 140 });
  expect(frameGeometry.commit).not.toHaveBeenCalled();
  expect(latestFrame().position).toMatchObject({ x: 0, y: 0 });
});
