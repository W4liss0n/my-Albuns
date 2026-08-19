import { act } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { threeSheetComposition } from "./albumCanvasTestFixtures";
import {
  continuousCanvasScale,
  createContinuousCanvasLayout,
} from "./canvasGeometry";
import {
  finishPixiInitialization,
  getPixiLifecycle,
  renderCanvas,
  setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();
const pixiLifecycle = getPixiLifecycle();

test("does not zoom the continuous Canvas outside sheet-editing mode", async () => {
  const onViewportChange = vi.fn();
  renderCanvas({ onViewportChange });
  await finishPixiInitialization();
  onViewportChange.mockClear();

  pixiLifecycle.instances[0].canvas.dispatchEvent(
    new WheelEvent("wheel", {
      cancelable: true,
      ctrlKey: true,
      deltaY: -120,
    }),
  );

  expect(onViewportChange).not.toHaveBeenCalled();
});

test("keeps edge sheets centered and tracks the centered sheet while scrolling", async () => {
  const onCenteredSheetChange = vi.fn();
  const onViewportChange = vi.fn();
  renderCanvas({
    compositionPlan: threeSheetComposition,
    onCenteredSheetChange,
    onViewportChange,
  });
  await finishPixiInitialization();

  const app = pixiLifecycle.instances[0];
  const scale = continuousCanvasScale(500, 300);
  const layout = createContinuousCanvasLayout(
    threeSheetComposition.sheets,
  );
  const entries = layout.entriesAtScale(scale);

  app.canvas.dispatchEvent(
    new WheelEvent("wheel", {
      cancelable: true,
      deltaY: 10_000,
    }),
  );

  const lastOffset =
    onViewportChange.mock.calls[
      onViewportChange.mock.calls.length - 1
    ]?.[0].offsetX;
  expect(lastOffset + entries[2].center * scale).toBeCloseTo(600, 4);
  expect(onCenteredSheetChange).toHaveBeenLastCalledWith("sheet-003");
  onCenteredSheetChange.mockClear();

  app.canvas.dispatchEvent(
    new WheelEvent("wheel", {
      cancelable: true,
      deltaY: -10_000,
    }),
  );

  const firstOffset =
    onViewportChange.mock.calls[
      onViewportChange.mock.calls.length - 1
    ]?.[0].offsetX;
  expect(firstOffset + entries[0].center * scale).toBeCloseTo(600, 4);
  expect(onCenteredSheetChange).not.toHaveBeenCalled();
});

test("resizes the Pixi renderer before fitting a taller Canvas", async () => {
  const onCanvasMetricsChange = vi.fn();
  renderCanvas({ onCanvasMetricsChange });
  await finishPixiInitialization();

  const host = document.querySelector(".canvas-host") as HTMLElement;
  Object.defineProperty(host, "clientWidth", {
    configurable: true,
    value: 900,
  });
  Object.defineProperty(host, "clientHeight", {
    configurable: true,
    value: 700,
  });
  await act(async () => {
    pixiLifecycle.resizeCallbacks[0]?.(
      [],
      {} as ResizeObserver,
    );
    await Promise.resolve();
  });

  const world = pixiLifecycle.instances[0].stage.children[0] as {
    scale: { x: number };
  };

  expect(pixiLifecycle.instances[0].resizeCount).toBe(1);
  expect(pixiLifecycle.instances[0].screen.width).toBe(900);
  expect(pixiLifecycle.instances[0].screen.height).toBe(700);
  const expectedScale = continuousCanvasScale(700, 300);
  expect(world.scale.x).toBeCloseTo(expectedScale, 4);
  expect(onCanvasMetricsChange).toHaveBeenLastCalledWith({
    width: 900,
    scale: expectedScale,
  });
});
