import { act, fireEvent } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { interactiveComposition } from "./albumCanvasTestFixtures";
import { displayWithLabel, finishPixiInitialization, getPixiLifecycle, renderCanvas, setupAlbumCanvasTestHarness } from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();

async function harness() {
  const frameGeometry = { disabled: false, dragThreshold: { x: 5, y: 5 }, preview: vi.fn(), commit: vi.fn(), onError: vi.fn() };
  const view = renderCanvas({ compositionPlan: interactiveComposition,
    mode: { kind: "sheet-editing", sheetId: "sheet-001" }, selectedFrameIds: ["frame-001"], frameGeometry });
  await finishPixiInitialization();
  const app = getPixiLifecycle().instances[0];
  vi.spyOn(app.canvas, "getBoundingClientRect").mockImplementation(() => ({
    x: 50, y: 60, left: 50, top: 60, right: 50 + app.screen.width, bottom: 60 + app.screen.height,
    width: app.screen.width, height: app.screen.height, toJSON: () => ({}),
  }));
  app.canvas.setPointerCapture = vi.fn();
  app.canvas.releasePointerCapture = vi.fn();
  const world = displayWithLabel("album-world");
  const camera = () => ({ x: world.position.x, y: world.position.y, scale: world.scale.x });
  const key = (key: string, extra = {}) => fireEvent.keyDown(app.canvas, { key, ctrlKey: true, ...extra });
  const wheel = (deltaY: number, x = 650, y = 310) => fireEvent.wheel(app.canvas, { ctrlKey: true, deltaY, clientX: x, clientY: y });
  const pan = (button = 1) => {
    if (button === 0) fireEvent.keyDown(app.canvas, { code: "Space", key: " " });
    fireEvent.pointerDown(app.canvas, { pointerId: 19, button, clientX: 650, clientY: 310 });
    fireEvent.pointerMove(window, { pointerId: 19, clientX: 700, clientY: 340 });
  };
  return { ...view, app, frameGeometry, world, camera, key, wheel, pan };
}

test("keyboard zoom uses the Canvas center, clamps at 4x and returns to fit through Ctrl+0 or the menu request", async () => {
  const view = await harness();
  const fitted = view.camera();
  view.key("+", { shiftKey: true });
  expect(view.world.scale.x).toBeCloseTo(fitted.scale * 1.2);
  expect(view.world.position.x + 300 * view.world.scale.x).toBeCloseTo(600);
  for (let count = 0; count < 20; count++) view.key("+");
  expect(view.world.scale.x).toBeCloseTo(fitted.scale * 4);
  view.key("0");
  expect(view.camera()).toEqual(fitted);
  view.key("-");
  expect(view.camera()).toEqual(fitted);
  view.key("=");
  view.rerenderCanvas({ editingNavigation: { disabled: false, fitRequest: 1 } });
  expect(view.camera()).toEqual(fitted);
  expect(view.frameGeometry.commit).not.toHaveBeenCalled();
  expect(view.onTransformCommit).not.toHaveBeenCalled();
  expect(view.onViewportChange).not.toHaveBeenCalled();
});

test("wheel keeps the Sheet point under the cursor and cancels browser zoom", async () => {
  const view = await harness();
  view.wheel(-500);
  const initial = view.camera();
  const sheetPoint = { x: (700 - initial.x) / initial.scale, y: (290 - initial.y) / initial.scale };
  const event = new WheelEvent("wheel", { bubbles: true, cancelable: true, ctrlKey: true,
    deltaY: -100, clientX: 750, clientY: 350 });
  act(() => view.app.canvas.dispatchEvent(event));
  expect(event.defaultPrevented).toBe(true);
  expect(view.world.position.x + sheetPoint.x * view.world.scale.x).toBeCloseTo(700);
  expect(view.world.position.y + sheetPoint.y * view.world.scale.y).toBeCloseTo(290);
});

test.each([0, 1])("Pan with button %s takes priority over Frame presses and leaves content unchanged", async (button) => {
  const view = await harness();
  view.wheel(-500);
  const initial = view.camera();
  const contentPress = vi.fn();
  view.app.canvas.addEventListener("pointerdown", contentPress, true);
  view.pan(button);
  expect(view.camera()).toEqual({ ...initial, x: initial.x + 50, y: initial.y + 30 });
  expect(contentPress).not.toHaveBeenCalled();
  expect(view.app.canvas).toHaveClass("pixi-canvas--navigation-pan");
  fireEvent.pointerUp(window, { pointerId: 20, clientX: 700, clientY: 340 });
  expect(view.app.canvas).toHaveClass("pixi-canvas--navigation-pan");
  fireEvent.pointerUp(window, { pointerId: 19, clientX: 700, clientY: 340 });
  expect(view.app.canvas).not.toHaveClass("pixi-canvas--navigation-pan");
  expect(view.onSelectFrame).not.toHaveBeenCalled();
  expect(view.frameGeometry.commit).not.toHaveBeenCalled();
  view.app.canvas.removeEventListener("pointerdown", contentPress, true);
});

test.each(["Escape", "blur", "cancel", "capture", "blocked"])("%s cancels Pan and a late release never selects content", async (reason) => {
  const view = await harness();
  view.wheel(-500);
  const initial = view.camera();
  view.pan();
  const exit = vi.fn();
  window.addEventListener("keydown", exit);
  if (reason === "Escape") fireEvent.keyDown(view.app.canvas, { key: "Escape" });
  else if (reason === "blur") fireEvent.blur(window);
  else if (reason === "cancel") fireEvent.pointerCancel(window, { pointerId: 19 });
  else if (reason === "capture") fireEvent(view.app.canvas, new PointerEvent("lostpointercapture", { pointerId: 19 }));
  else view.rerenderCanvas({ editingNavigation: { disabled: true, fitRequest: 0 } });
  expect(view.camera()).toEqual(initial);
  expect(exit).not.toHaveBeenCalled();
  window.removeEventListener("keydown", exit);
  fireEvent.pointerUp(window, { pointerId: 19, clientX: 750, clientY: 380 });
  expect(view.camera()).toEqual(initial);
  expect(view.onSelectFrame).not.toHaveBeenCalled();
});

test("typing, modal controls, repeat and blocked commands never change the camera", async () => {
  const view = await harness();
  const initial = view.camera();
  const input = document.createElement("input");
  document.body.appendChild(input);
  fireEvent.keyDown(input, { key: "+", ctrlKey: true });
  input.remove();
  view.key("+", { repeat: true });
  expect(view.camera()).toEqual(initial);
  view.rerenderCanvas({ editingNavigation: { disabled: true, fitRequest: 0 } });
  view.key("+");
  view.wheel(-500);
  view.pan();
  expect(view.camera()).toEqual(initial);
});

test("leaving or changing the edited Sheet resets the camera; normal mode has no zoom", async () => {
  const view = await harness();
  const fitted = view.camera();
  view.wheel(-500);
  view.rerenderCanvas({ mode: { kind: "normal" } });
  const normal = view.camera();
  view.wheel(-500);
  view.key("+");
  expect(view.camera()).toEqual(normal);
  view.rerenderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" } });
  expect(view.camera()).toEqual(fitted);
  view.wheel(-500);
  view.rerenderCanvas({ projectId: "another-project" });
  expect(view.camera()).toEqual(fitted);
});

test("resize preserves the magnification and Pan never loses the entire Sheet", async () => {
  const view = await harness();
  view.wheel(-500);
  const zoom = view.app.canvas.dataset.editingZoom;
  view.app.screen.width = 700;
  view.rerenderCanvas({});
  expect(view.app.canvas.dataset.editingZoom).toBe(zoom);
  view.pan();
  fireEvent.pointerMove(window, { pointerId: 19, clientX: 50_000, clientY: 50_000 });
  expect(view.world.position.x).toBe(28);
  expect(view.world.position.y).toBe(28);
  fireEvent.pointerMove(window, { pointerId: 19, clientX: -50_000, clientY: -50_000 });
  expect(view.world.position.x + 600 * view.world.scale.x).toBeCloseTo(700 - 28);
  expect(view.world.position.y + 300 * view.world.scale.y).toBeCloseTo(500 - 28);
});

test("area selection uses the zoomed coordinates and zoom cancels an unfinished area", async () => {
  const view = await harness();
  const onSelectFrames = vi.fn();
  view.rerenderCanvas({ onSelectFrames });
  view.wheel(-500);
  const point = (x: number, y: number) => ({ clientX: 50 + view.world.position.x + x * view.world.scale.x,
    clientY: 60 + view.world.position.y + y * view.world.scale.y });
  const start = () => act(() => view.app.stage.emit("pointerdown", { target: view.app.stage,
    button: 0, pointerId: 7, ctrlKey: false, ...point(0, 0), stopPropagation: vi.fn() }));
  start();
  fireEvent.pointerUp(window, { pointerId: 7, ...point(200, 150) });
  expect(onSelectFrames).toHaveBeenCalledWith(["frame-001"]);
  onSelectFrames.mockClear();
  start();
  fireEvent.pointerMove(window, { pointerId: 7, ...point(200, 150) });
  view.wheel(-100);
  expect(displayWithLabel("frame-area-selection").visible).toBe(false);
  fireEvent.pointerUp(window, { pointerId: 7, ...point(200, 150) });
  expect(onSelectFrames).not.toHaveBeenCalled();
});
