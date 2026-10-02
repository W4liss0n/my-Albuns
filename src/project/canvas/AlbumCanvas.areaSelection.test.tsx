import { act, fireEvent } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { interactiveComposition } from "./albumCanvasTestFixtures";
import { displayWithLabel, finishPixiInitialization, getPixiLifecycle, renderCanvas, setupAlbumCanvasTestHarness } from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();

function latest(label: string) {
  const nodes = getPixiLifecycle().displays.filter((node) => node.label === label);
  return nodes[nodes.length - 1];
}

async function harness({ mode = "sheet-editing", locked = false, disabled = false }:
  { mode?: "sheet-editing" | "normal"; locked?: boolean; disabled?: boolean } = {}) {
  const composition = structuredClone(interactiveComposition);
  const source = composition.sheets[0].frames[0];
  composition.sheets[0].frames = [
    { ...source, frameId: "a", photo: null, clipRect: { x: 20_000, y: 20_000, width: 60_000, height: 80_000 } },
    { ...source, frameId: "b", clipRect: { x: 200_000, y: 80_000, width: 60_000, height: 120_000 } },
    { ...source, frameId: "c", clipRect: { x: 230_000, y: 120_000, width: 70_000, height: 80_000 } },
  ];
  const frameGeometry = { disabled, dragThreshold: { x: 5, y: 5 }, preview: vi.fn(), commit: vi.fn(), onError: vi.fn() };
  const view = renderCanvas({ compositionPlan: composition, mode: mode === "normal" ? { kind: "normal" } :
    { kind: "sheet-editing", sheetId: "sheet-001" }, selectedFrameIds: ["a"], frameGeometry });
  await finishPixiInitialization();
  const onSelectFrames = vi.fn();
  view.rerenderCanvas({ onSelectFrames, sheetBarMetadata: [{ sheetId: "sheet-001", pageNumbers: [1, 2], layoutLocked: locked }] });
  const app = getPixiLifecycle().instances[0];
  vi.spyOn(app.canvas, "getBoundingClientRect").mockReturnValue({
    x: 0, y: 0, left: 0, top: 0, right: app.screen.width, bottom: app.screen.height,
    width: app.screen.width, height: app.screen.height, toJSON: () => ({}),
  });
  app.canvas.setPointerCapture = vi.fn();
  app.canvas.releasePointerCapture = vi.fn();
  const point = (x: number, y: number) => {
    const world = displayWithLabel("album-world");
    const sheet = displayWithLabel("canvas-sheet-sheet-001");
    return { clientX: world.position.x + (sheet.position.x + x) * world.scale.x,
      clientY: world.position.y + (sheet.position.y + y) * world.scale.y };
  };
  const start = (x: number, y: number, ctrlKey = false) => act(() => app.stage.emit("pointerdown", {
    target: app.stage, button: 0, pointerId: 7, ...point(x, y), ctrlKey, stopPropagation: vi.fn(),
  }));
  const move = (x: number, y: number, ctrlKey = false) => fireEvent.pointerMove(window, { pointerId: 7, ...point(x, y), ctrlKey });
  const finish = (x: number, y: number, ctrlKey = false) => fireEvent.pointerUp(window, { pointerId: 7, ...point(x, y), ctrlKey });
  return { ...view, app, onSelectFrames, frameGeometry, composition, start, move, finish };
}

test.each([false, true])("area selects intersecting Photos and placeholders in either direction (reverse=%s)", async (reverse) => {
  const view = await harness();
  const start = reverse ? [210, 90] : [0, 0];
  const end = reverse ? [0, 0] : [210, 90];
  view.start(start[0], start[1]);
  view.move(end[0], end[1]);
  expect(displayWithLabel("frame-area-selection").visible).toBe(true);
  expect(displayWithLabel("frame-selection-container-b").visible).toBe(true);
  expect(displayWithLabel("frame-selection-container-c").visible).toBe(false);
  expect(view.onSelectFrames).not.toHaveBeenCalled();
  view.finish(end[0], end[1]);
  expect(view.onSelectFrames).toHaveBeenCalledExactlyOnceWith(["a", "b"]);
  expect(displayWithLabel("frame-area-selection").visible).toBe(false);
  expect(view.frameGeometry.commit).not.toHaveBeenCalled();
  expect(view.frameGeometry.preview).not.toHaveBeenCalled();
});

test("Ctrl adds the intersected Frames, includes overlapping Frames and responds during the gesture", async () => {
  const view = await harness({ locked: true });
  view.start(190, 70);
  view.move(250, 150);
  expect(latest("frame-selection-container-a").visible).toBe(false);
  fireEvent.keyDown(window, { key: "Control", ctrlKey: true });
  expect(latest("frame-selection-container-a").visible).toBe(true);
  view.finish(250, 150, true);
  expect(view.onSelectFrames).toHaveBeenCalledExactlyOnceWith(["a", "b", "c"]);
});

test("release consumes the synthesized tap instead of clearing or collapsing the area selection", async () => {
  const view = await harness();
  view.start(0, 0);
  view.finish(210, 90);
  const sheet = displayWithLabel("canvas-sheet-sheet-001");
  act(() => sheet.emit("pointertap", { target: sheet, button: 0, detail: 1 }));
  act(() => displayWithLabel("canvas-frame-b").emit("pointertap", { button: 0, detail: 1, stopPropagation: vi.fn() }));
  expect(view.onSelectFrames).toHaveBeenCalledExactlyOnceWith(["a", "b"]);
  expect(view.onSelectFrame).not.toHaveBeenCalled();
});

test.each(["Escape", "blur", "cancel", "capture", "selection", "composition"])("%s cancels the area without changing the confirmed selection", async (reason) => {
  const view = await harness();
  view.start(190, 70);
  view.move(250, 150);
  if (reason === "Escape") {
    const exitMode = vi.fn();
    window.addEventListener("keydown", exitMode);
    const event = new KeyboardEvent("keydown", { key: "Escape", cancelable: true, bubbles: true });
    act(() => window.dispatchEvent(event));
    window.removeEventListener("keydown", exitMode);
    expect(exitMode).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
  } else if (reason === "blur") fireEvent.blur(window);
  else if (reason === "cancel") fireEvent.pointerCancel(window, { pointerId: 7 });
  else if (reason === "capture") fireEvent(view.app.canvas, new PointerEvent("lostpointercapture", { pointerId: 7 }));
  else if (reason === "selection") view.rerenderCanvas({ selectedFrameIds: ["c"] });
  else view.rerenderCanvas({ composition: structuredClone(view.composition) });
  view.finish(250, 150);
  expect(view.onSelectFrames).not.toHaveBeenCalled();
  expect(displayWithLabel("frame-area-selection").visible).toBe(false);
});

test("releasing the mouse after Escape does not turn the cancelled drag into an empty click", async () => {
  const view = await harness();
  view.start(0, 0);
  view.move(210, 90);
  fireEvent.keyDown(window, { key: "Escape" });
  await act(async () => { await new Promise((resolve) => setTimeout(resolve, 5)); });
  view.finish(210, 90);
  const sheet = displayWithLabel("canvas-sheet-sheet-001");
  act(() => sheet.emit("pointertap", { target: sheet, button: 0, detail: 1 }));
  expect(view.onSelectFrames).not.toHaveBeenCalled();
  expect(view.onSelectFrame).not.toHaveBeenCalled();
});

test.each([{ mode: "normal" as const }, { disabled: true }])("area is inactive for %o", async (options) => {
  const view = await harness(options);
  view.start(0, 0);
  view.finish(250, 150);
  expect(view.onSelectFrames).not.toHaveBeenCalled();
  expect(displayWithLabel("frame-area-selection").visible).toBe(false);
});

test("a small movement stays a click, and a different pointer cannot finish the area", async () => {
  const view = await harness();
  view.start(0, 0);
  fireEvent.pointerUp(window, { pointerId: 8, clientX: 800, clientY: 400 });
  expect(view.onSelectFrames).not.toHaveBeenCalled();
  view.finish(1, 1);
  expect(view.onSelectFrames).toHaveBeenCalledExactlyOnceWith([]);
  expect(displayWithLabel("frame-area-selection").visible).toBe(false);
});
