import { act, render } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { threeSheetComposition } from "./albumCanvasTestFixtures";
import {
  continuousCanvasScale,
  createContinuousCanvasLayout,
} from "./canvasGeometry";
import {
  AlbumCanvas,
  displayWithLabel,
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

test("centers the edited Sheet in the first normal Canvas update", async () => {
  const onCanvasMetricsChange = vi.fn();
  const layout = createContinuousCanvasLayout(
    threeSheetComposition.sheets,
  );
  const callbacks = {
    onSelectFrame: vi.fn(),
    onEditSheet: vi.fn(),
    onFocusSheet: vi.fn(),
    onCenteredSheetChange: vi.fn(),
    onViewportChange: vi.fn(),
    onTransformPreview: vi.fn(),
    onTransformCommit: vi.fn(async () => true),
  };
  const canvas = (
    mode:
      | { kind: "normal" }
      | { kind: "sheet-editing"; sheetId: string },
    centeredSheetId: string,
  ) => (
    <AlbumCanvas
      projectId="project-spike-001"
      mode={mode}
      composition={threeSheetComposition}
      sheetBarMetadata={[]}
      continuousCanvasLayout={layout}
      selectedFrameId={null}
      focusedSheetId="sheet-002"
      centeredSheetId={centeredSheetId}
      viewport={{ offsetX: 42 }}
      onCanvasMetricsChange={onCanvasMetricsChange}
      {...callbacks}
    />
  );

  const view = render(
    canvas({ kind: "sheet-editing", sheetId: "sheet-003" }, "sheet-002"),
  );
  await finishPixiInitialization();
  onCanvasMetricsChange.mockClear();
  callbacks.onViewportChange.mockClear();
  const host = document.querySelector(".canvas-host") as HTMLElement;
  Object.defineProperty(host, "clientHeight", {
    configurable: true,
    value: 400,
  });

  view.rerender(canvas({ kind: "normal" }, "sheet-003"));

  const app = pixiLifecycle.instances[0];
  const world = app.stage.children[0] as {
    position: { x: number };
  };
  const expectedScale = continuousCanvasScale(400, 300);
  const expectedOffset = layout.centeredOffset(
    "sheet-003",
    expectedScale,
    app.screen.width,
  );
  expect(world.position.x).toBeCloseTo(expectedOffset ?? 0, 4);
  expect(callbacks.onViewportChange).toHaveBeenLastCalledWith({
    offsetX: expectedOffset,
  });
  expect(onCanvasMetricsChange).toHaveBeenCalledOnce();
});

test("isolates the target sheet and suppresses continuous navigation in Sheet Edit Mode", async () => {
  const onViewportChange = vi.fn();
  renderCanvas({
    compositionPlan: threeSheetComposition,
    mode: { kind: "sheet-editing", sheetId: "sheet-002" },
    onViewportChange,
  });
  await finishPixiInitialization();

  expect(displayWithLabel("canvas-sheet-sheet-002")).toBeDefined();
  expect(
    pixiLifecycle.displays.some(
      ({ label }) => label === "canvas-sheet-sheet-001",
    ),
  ).toBe(false);
  expect(
    pixiLifecycle.displays.some(
      ({ label }) => label === "canvas-sheet-sheet-003",
    ),
  ).toBe(false);
  onViewportChange.mockClear();

  pixiLifecycle.instances[0].canvas.dispatchEvent(
    new WheelEvent("wheel", {
      cancelable: true,
      deltaY: 600,
    }),
  );

  expect(onViewportChange).not.toHaveBeenCalled();
});
