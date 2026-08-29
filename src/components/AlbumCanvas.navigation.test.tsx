import { act, fireEvent, render, screen } from "@testing-library/react";
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
      bubbles: true,
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
      bubbles: true,
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
      bubbles: true,
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

test("keeps horizontal wheel navigation alive across the left, center, and right Sheet Bar regions", async () => {
  const onViewportChange = vi.fn();
  renderCanvas({
    compositionPlan: threeSheetComposition,
    onViewportChange,
    sheetReorder: {
      disabled: false,
      onCancel: vi.fn(),
      onDrop: vi.fn(),
      onNavigate: vi.fn(),
      onPreview: vi.fn(),
      representation: {
        ghost: null,
        order: threeSheetComposition.sheets.map((sheet) => sheet.sheetId),
        placeholderIndex: null,
      },
      status: "idle",
    },
  });
  await finishPixiInitialization();

  const pixiBackedBarTarget = pixiLifecycle.instances[0].canvas;
  const rightBarTarget = screen.getByRole("button", {
    name: "Reordenar Lâmina 01 pela Barra",
  });
  // The left/page and central/action regions belong to the Pixi canvas. The
  // free right region is the productive DOM reorder overlay that exposed the
  // original propagation asymmetry.
  const targets = [
    { name: "left", target: pixiBackedBarTarget, clientX: 160 },
    { name: "center", target: pixiBackedBarTarget, clientX: 300 },
    { name: "right", target: rightBarTarget, clientX: 440 },
  ];

  for (const { name, target, clientX } of targets) {
    for (const deltaY of [-120, 120]) {
      onViewportChange.mockClear();
      target.dispatchEvent(
        new WheelEvent("wheel", {
          bubbles: true,
          cancelable: true,
          clientX,
          deltaY,
        }),
      );

      expect(
        onViewportChange,
        `${name} Bar region with deltaY ${deltaY}`,
      ).toHaveBeenCalledOnce();
      const nextOffset = onViewportChange.mock.calls[0]?.[0].offsetX;
      expect(Math.sign(nextOffset - 42)).toBe(-Math.sign(deltaY));
    }
  }
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

test("exposes a horizontal scrollbar bound to the continuous Canvas viewport", async () => {
  const onCenteredSheetChange = vi.fn();
  const onViewportChange = vi.fn();
  renderCanvas({
    compositionPlan: threeSheetComposition,
    onCenteredSheetChange,
    onViewportChange,
  });
  await finishPixiInitialization();

  const scrollbar = screen.getByRole("scrollbar", {
    name: "Navegação horizontal das Lâminas",
  });
  expect(scrollbar).toHaveAttribute("aria-orientation", "horizontal");

  Object.defineProperty(scrollbar, "scrollLeft", {
    configurable: true,
    value: 900,
    writable: true,
  });
  fireEvent.scroll(scrollbar);

  const layout = createContinuousCanvasLayout(
    threeSheetComposition.sheets,
  );
  const scale = continuousCanvasScale(500, 300);
  const maximum = layout.centeredOffset(
    "sheet-001",
    scale,
    1_200,
  );
  expect(maximum).not.toBeNull();
  expect(onViewportChange).toHaveBeenLastCalledWith({
    offsetX: (maximum ?? 0) - 900,
  });
  expect(onCenteredSheetChange).toHaveBeenCalled();
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
      bubbles: true,
      cancelable: true,
      deltaY: 600,
    }),
  );

  expect(onViewportChange).not.toHaveBeenCalled();
});
