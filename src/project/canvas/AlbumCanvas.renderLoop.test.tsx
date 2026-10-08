import { act, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { LogEvent } from "../../application/logging";
import type { CanvasSheetReorder } from "./albumCanvasContract";
import { threeSheetComposition } from "./albumCanvasTestFixtures";
import {
  advancePixiTicker,
  dispatchTrustedEvent,
  displayWithLabel,
  finishPixiInitialization,
  getPixiLifecycle,
  renderCanvas,
  setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();
const pixiLifecycle = getPixiLifecycle();

let clock = 0;

async function renderIdleCanvas(options: Parameters<typeof renderCanvas>[0] = {}) {
  clock = 0;
  vi.spyOn(performance, "now").mockImplementation(() => clock);
  const view = renderCanvas(options);
  await finishPixiInitialization();
  const app = pixiLifecycle.instances[0];
  await settle();
  return { view, app };
}

async function settle() {
  clock += 10_000;
  await advancePixiTicker(16);
}

test("renders after an update and then stops every ticker", async () => {
  vi.spyOn(performance, "now").mockImplementation(() => clock);
  const view = renderCanvas();
  await finishPixiInitialization();
  const app = pixiLifecycle.instances[0];
  const system = pixiLifecycle.systemTicker!;

  expect(app.ticker.started).toBe(true);
  await advancePixiTicker(16);
  expect(app.renderCount).toBe(1);

  await settle();
  expect(app.renderCount).toBe(2);
  expect(app.ticker.started).toBe(false);
  expect(system.started).toBe(false);
  await advancePixiTicker(16);
  expect(app.renderCount).toBe(2);

  view.rerenderCanvas({ selectedFrameIds: ["frame-001"] });
  expect(app.ticker.started).toBe(true);
  expect(system.started).toBe(true);
  await advancePixiTicker(16);
  expect(app.renderCount).toBe(3);

  view.unmount();
  expect(system.started).toBe(true);
});

test("pointer input on the Canvas wakes it and a press holds it until release", async () => {
  const { app } = await renderIdleCanvas();
  expect(app.ticker.started).toBe(false);

  dispatchTrustedEvent(app.canvas, new PointerEvent("pointermove", { bubbles: true }));
  expect(app.ticker.started).toBe(true);
  await settle();
  expect(app.ticker.started).toBe(false);

  // Pixi's resting-pointer hover events are synthetic and must not keep it awake.
  app.canvas.dispatchEvent(new PointerEvent("pointermove", { bubbles: true }));
  document.dispatchEvent(new PointerEvent("pointermove"));
  expect(app.ticker.started).toBe(false);

  dispatchTrustedEvent(app.canvas, new PointerEvent("pointerdown", { bubbles: true, buttons: 1 }));
  await settle();
  await settle();
  expect(app.ticker.started).toBe(true);
  dispatchTrustedEvent(app.canvas, new PointerEvent("pointerup", { bubbles: true, buttons: 0 }));
  await advancePixiTicker(16);
  expect(app.ticker.started).toBe(true);
  await settle();
  expect(app.ticker.started).toBe(false);
});

test("a Sheet Bar fade started by Pixi hover keeps rendering until it ends", async () => {
  const { app } = await renderIdleCanvas();
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });

  displayWithLabel("canvas-sheet-sheet-001").emit("pointerenter", {});
  expect(app.ticker.started).toBe(false);
  let steps = 0;
  while (vi.getTimerCount() > 0 && steps < 50) {
    vi.advanceTimersByTime(16);
    steps += 1;
    // Each step wakes the loop even after it went idle since the last one.
    expect(app.ticker.started, `step ${steps}`).toBe(true);
    clock += 10_000;
    const renders = app.renderCount;
    await advancePixiTicker(16);
    expect(app.renderCount, `step ${steps}`).toBe(renders + 1);
    expect(app.ticker.started).toBe(false);
  }
  expect(steps).toBeGreaterThan(1);
});

test("a Sheet reorder animation holds the loop past the idle delay until it ends", async () => {
  const idleReorder: CanvasSheetReorder = {
    disabled: false,
    onCancel: vi.fn(),
    onDrop: vi.fn(),
    onSelect: vi.fn(),
    onPreview: vi.fn(),
    representation: { ghost: null, order: ["sheet-001", "sheet-002", "sheet-003"], placeholderIndex: null },
    status: "idle",
  };
  const { view, app } = await renderIdleCanvas({
    compositionPlan: threeSheetComposition,
    sheetReorder: idleReorder,
  });
  const second = displayWithLabel("canvas-sheet-sheet-002");
  const third = displayWithLabel("canvas-sheet-sheet-003");
  const [from, to] = [second.position.x, third.position.x];

  view.rerenderCanvas({
    sheetReorder: {
      ...idleReorder,
      representation: {
        ghost: { sheetId: "sheet-003" },
        order: ["sheet-001", "sheet-003", "sheet-002"],
        placeholderIndex: 1,
      },
      status: "preview",
    },
  });
  // Frames slower than the idle delay: only the hold keeps them coming.
  clock += 10_000;
  await advancePixiTicker(70);
  expect(second.position.x).toBeGreaterThan(from);
  expect(second.position.x).toBeLessThan(to);
  expect(app.ticker.started).toBe(true);

  clock += 10_000;
  await advancePixiTicker(70);
  expect(second.position.x).toBe(to);
  // The release wakes the loop for the frames that follow the last step.
  expect(app.ticker.started).toBe(true);
  await settle();
  expect(app.ticker.started).toBe(false);
});

test("a context restored after the loop went idle redraws on the next frame", async () => {
  const logEvents: LogEvent[] = [];
  const { view, app } = await renderIdleCanvas({ logger: { write: (event) => logEvents.push(event) } });

  act(() => {
    app.canvas.dispatchEvent(new Event("webglcontextlost", { cancelable: true }));
  });
  expect(view.getByRole("status")).toHaveTextContent("Restaurando editor");
  await settle();
  expect(app.ticker.started).toBe(false);

  act(() => {
    app.canvas.dispatchEvent(new Event("webglcontextrestored"));
  });
  expect(app.ticker.started).toBe(true);
  await advancePixiTicker(16);
  await waitFor(() => expect(view.queryByRole("status")).not.toBeInTheDocument());
  expect(logEvents).toEqual(expect.arrayContaining([
    expect.objectContaining({ event: "canvas_context_restored" }),
  ]));
});
