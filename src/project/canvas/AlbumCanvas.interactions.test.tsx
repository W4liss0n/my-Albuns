import { act, render } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import {
  AlbumCanvas,
  displayWithHandler,
  displayWithLabel,
  finishPixiInitialization,
  getPixiLifecycle,
  latestDisplayWithHandler,
  renderCanvas,
  setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

import type { CompositionPlan } from "../../domain/project";
import type { PhotoTransformDelta } from "./AlbumCanvas";
import {
  interactiveComposition,
  pannedInteractiveComposition,
  rotatedInteractiveComposition,
} from "./albumCanvasTestFixtures";
import { createContinuousCanvasLayout } from "./canvasGeometry";

setupAlbumCanvasTestHarness();
const pixiLifecycle = getPixiLifecycle();

test("reveals the dimmed Photo overflow and thirds guides only during Pan", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const onTransformPreview = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  expect(frame.cursor).toBe("default");
  const outsidePreview = displayWithLabel(
    "photo-pan-outside-preview",
  );
  const insidePreview = displayWithLabel(
    "photo-pan-inside-preview",
  );
  const thirdsGuides = displayWithLabel("photo-pan-thirds-guides");
  const originalPosition = { ...insidePreview.position };

  expect(outsidePreview.visible).toBe(false);
  expect(outsidePreview.alpha).toBeGreaterThan(0);
  expect(outsidePreview.alpha).toBeLessThan(1);
  expect(insidePreview.alpha).toBe(1);
  expect(thirdsGuides.visible).toBe(false);
  expect(thirdsGuides.pathCommands).toEqual([
    { kind: "moveTo", x: 100, y: 0 },
    { kind: "lineTo", x: 100, y: 200 },
    { kind: "moveTo", x: 200, y: 0 },
    { kind: "lineTo", x: 200, y: 200 },
    { kind: "moveTo", x: 0, y: 200 / 3 },
    { kind: "lineTo", x: 300, y: 200 / 3 },
    { kind: "moveTo", x: 0, y: 400 / 3 },
    { kind: "lineTo", x: 300, y: 400 / 3 },
  ]);

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });

  expect(outsidePreview.visible).toBe(true);
  expect(thirdsGuides.visible).toBe(true);

  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  expect(outsidePreview.position).toEqual(insidePreview.position);

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 0, y: 0 },
  });

  expect(outsidePreview.visible).toBe(false);
  expect(thirdsGuides.visible).toBe(false);
  expect(onTransformCommit).not.toHaveBeenCalled();
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  pixiLifecycle.instances[0].stage.emit("pointercancel", {});

  expect(outsidePreview.visible).toBe(false);
  expect(thirdsGuides.visible).toBe(false);
  expect(insidePreview.position).toEqual(originalPosition);
  expect(outsidePreview.position).toEqual(originalPosition);
  expect(onTransformCommit).not.toHaveBeenCalled();
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);
});

test("keeps the photo inside a stationary frame mask throughout pan", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  const maskedViewport = displayWithLabel(
    frame.label.replace("canvas-frame-", "frame-content-"),
  ).children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as
    | {
        children: unknown[];
        position: { x: number; y: number };
      }
    | undefined;

  expect(maskedViewport).toBeDefined();
  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 1_000, y: 0 },
  });

  const photoLayer = maskedViewport?.children.find(
    (child) => (child as { label?: string }).label === "photo-pan-inside-preview",
  ) as { position: { x: number; y: number } } | undefined;

  expect(maskedViewport?.position).toEqual({ x: 0, y: 0 });
  expect(photoLayer).toBeDefined();
  expect(photoLayer?.position.x).toBeLessThanOrEqual(200);

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 1_000, y: 0 },
  });

  expect(frame.cursor).toBe("default");

  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(onTransformCommit).toHaveBeenCalledWith({
    frameId: "frame-001",
    deltaPanX: 1,
    deltaPanY: 0,
    deltaZoom: 0,
  });
});

test("reports the live Photo transform while Pan is moving", async () => {
  const onTransformPreview = vi.fn();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformPreview,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });

  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: expect.any(Number),
    panY: 0,
    zoom: 1,
  });
  expect(
    onTransformPreview.mock.calls[
      onTransformPreview.mock.calls.length - 1
    ]?.[0]?.panX,
  ).toBeGreaterThan(0);
  expect(onTransformCommit).not.toHaveBeenCalled();
});

test("keeps a crash-interrupted continuous gesture in memory without committing it", async () => {
  const onTransformPreview = vi.fn();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const view = renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformPreview,
    onTransformCommit,
  });
  await finishPixiInitialization();

  displayWithLabel("canvas-frame-frame-001").emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  expect(onTransformPreview).toHaveBeenCalled();
  expect(onTransformCommit).not.toHaveBeenCalled();

  view.unmount();

  expect(onTransformCommit).not.toHaveBeenCalled();
});

test("keeps every frame corner covered while panning a rotated photo", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: rotatedInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  const maskedViewport = displayWithLabel(
    frame.label.replace("canvas-frame-", "frame-content-"),
  ).children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as { children: unknown[] };
  const photoLayer = maskedViewport.children.find(
    (child) => (child as { label?: string }).label === "photo-pan-inside-preview",
  ) as {
    position: { x: number; y: number };
    pivot: { x: number; y: number };
    rotation: number;
    scale: { x: number; y: number };
  };

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 10_000, y: 10_000 },
  });

  const cosine = Math.cos(photoLayer.rotation);
  const sine = Math.sin(photoLayer.rotation);
  for (const [cornerX, cornerY] of [
    [0, 0],
    [300, 0],
    [0, 200],
    [300, 200],
  ]) {
    const deltaX = cornerX - photoLayer.position.x;
    const deltaY = cornerY - photoLayer.position.y;
    const localX =
      (cosine * deltaX + sine * deltaY) /
      Math.abs(photoLayer.scale.x);
    const localY =
      (-sine * deltaX + cosine * deltaY) /
      Math.abs(photoLayer.scale.y);

    expect(Math.abs(localX)).toBeLessThanOrEqual(
      photoLayer.pivot.x + 0.001,
    );
    expect(Math.abs(localY)).toBeLessThanOrEqual(
      photoLayer.pivot.y + 0.001,
    );
  }

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 10_000, y: 10_000 },
  });

  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(onTransformCommit).toHaveBeenCalledWith({
    frameId: "frame-001",
    deltaPanX: 1,
    deltaPanY: -1,
    deltaZoom: 0,
  });
});

test("does not reset an active Pan preview when wheel Zoom starts", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  const maskedViewport = displayWithLabel(
    frame.label.replace("canvas-frame-", "frame-content-"),
  ).children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as { children: unknown[] };
  const photoLayer = maskedViewport.children.find(
    (child) => (child as { label?: string }).label === "photo-pan-inside-preview",
  ) as {
    position: { x: number; y: number };
    scale: { x: number; y: number };
  };

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  const pannedX = photoLayer.position.x;
  expect(pannedX).toBeGreaterThan(150);

  frame.emit("wheel", {
    altKey: true,
    deltaY: -100,
    preventDefault: vi.fn(),
  });

  expect(photoLayer.scale.y).toBeGreaterThan(1);
  expect(photoLayer.position.x).toBeCloseTo(pannedX, 4);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).not.toHaveBeenCalled();

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 40, y: 0 },
  });

  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(onTransformCommit.mock.calls[0][0]).toMatchObject({
    frameId: "frame-001",
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.12, 6),
  });
  expect(
    onTransformCommit.mock.calls[0][0].deltaPanX,
  ).toBeGreaterThan(0);
});

test("previews a smooth wheel zoom and commits the sequence once", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const onTransformPreview = vi.fn();
  renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("wheel");
  const maskedDisplay = displayWithLabel(
    frame.label.replace("canvas-frame-", "frame-content-"),
  ).children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as {
    children: unknown[];
    position: { x: number };
    scale: { y: number };
  };
  const photoLayer =
    (maskedDisplay.children.find(
      (child) => (child as { label?: string }).label === "photo-pan-inside-preview",
    ) as
      | { position: { x: number }; scale: { y: number } }
      | undefined) ?? maskedDisplay;

  const wheel = () =>
    frame.emit("wheel", {
      altKey: true,
      deltaY: -100,
      preventDefault: vi.fn(),
    });

  wheel();
  expect(photoLayer.scale.y).toBeGreaterThan(1);
  expect(photoLayer.position.x).toBeCloseTo(83.4, 1);
  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: -0.9,
    panY: 0,
    zoom: expect.any(Number),
  });
  expect(
    onTransformPreview.mock.calls[
      onTransformPreview.mock.calls.length - 1
    ]?.[0]?.zoom,
  ).toBeGreaterThan(1);
  expect(onTransformCommit).not.toHaveBeenCalled();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(300);
  });
  wheel();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(300);
  });
  wheel();

  expect(onTransformCommit).not.toHaveBeenCalled();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });

  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(onTransformCommit.mock.calls[0][0]).toMatchObject({
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
  });
  expect(
    onTransformCommit.mock.calls[0][0].deltaZoom,
  ).toBeGreaterThan(0);
});

test("rolls the Pixi preview back when the Project rejects a transform", async () => {
  let resolveCommit!: (accepted: boolean) => void;
  const commitResult = new Promise<boolean>((resolve) => {
    resolveCommit = resolve;
  });
  const onTransformCommit = vi.fn(
    (_delta: PhotoTransformDelta) => commitResult,
  );
  const onTransformPreview = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  const insidePreview = displayWithLabel(
    "photo-pan-inside-preview",
  );
  const originalPosition = { ...insidePreview.position };

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 40, y: 0 },
  });

  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(insidePreview.position.x).toBeGreaterThan(
    originalPosition.x,
  );

  await act(async () => {
    resolveCommit(false);
    await commitResult;
  });

  expect(insidePreview.position).toEqual(originalPosition);
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);
});

test("cancels pending Pan and Zoom gestures when the Project changes", async () => {
  vi.useFakeTimers();
  const commitA = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const commitB = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const commitC = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const commonProps = {
    mode: { kind: "normal" } as const,
    composition: interactiveComposition,
    sheetBarMetadata: [],
    continuousCanvasLayout: createContinuousCanvasLayout(
      interactiveComposition.sheets,
    ),
    selectedFrameIds: [],
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 42 },
    onSelectFrame: vi.fn(),
    onEditSheet: vi.fn(),
    onFocusSheet: vi.fn(),
    onCenteredSheetChange: vi.fn(),
    onViewportChange: vi.fn(),
    onTransformPreview: vi.fn(),
  };
  const canvas = (
    projectId: string,
    onTransformCommit: (
      delta: PhotoTransformDelta,
    ) => Promise<boolean>,
  ) => (
    <AlbumCanvas
      {...commonProps}
      projectId={projectId}
      onTransformCommit={onTransformCommit}
    />
  );

  const view = render(canvas("project-a", commitA));
  await finishPixiInitialization();
  const world = pixiLifecycle.instances[0].stage.children[0] as {
    children: unknown[];
  };
  const sheetA = world.children[0];

  displayWithHandler("wheel").emit("wheel", {
    altKey: true,
    deltaY: -100,
    preventDefault: vi.fn(),
  });
  view.rerender(canvas("project-b", commitB));
  const sheetB = world.children[0];

  expect(sheetB).not.toBe(sheetA);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(commitA).not.toHaveBeenCalled();
  expect(commitB).not.toHaveBeenCalled();

  latestDisplayWithHandler("pointerdown").emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  view.rerender(canvas("project-c", commitC));
  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 40, y: 0 },
  });

  expect(world.children[0]).not.toBe(sheetB);
  expect(commitB).not.toHaveBeenCalled();
  expect(commitC).not.toHaveBeenCalled();
});

test("does not route Alt Pan or Zoom to a Photo while editing the sheet", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: interactiveComposition,
    mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    onTransformCommit,
  });
  await finishPixiInitialization();

  displayWithHandler("wheel").emit("wheel", {
    altKey: true,
    deltaY: -100,
    preventDefault: vi.fn(),
  });
  latestDisplayWithHandler("pointerdown").emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 40, y: 0 },
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });

  expect(onTransformCommit).not.toHaveBeenCalled();
});

// A portrait Photo (300 x 400 mm at fill) in the 300 x 200 mm Frame: the only
// slack is vertical, 100 mm on each side of the centre.
const portraitInteractiveComposition: CompositionPlan = {
  frameBorder: { kind: "none" },
  sheets: [
    {
      ...interactiveComposition.sheets[0],
      frames: [
        {
          ...interactiveComposition.sheets[0].frames[0],
          photo: {
            ...interactiveComposition.sheets[0].frames[0].photo!,
            drawRect: { x: 0, y: -100_000, width: 300_000, height: 400_000 },
            placement: {
              ...interactiveComposition.sheets[0].frames[0].photo!.placement,
              current: {
                center: { x: 150_000, y: 100_000 },
                size: { width: 300_000, height: 400_000 },
              },
              panToCenter: { xx: 0, xy: 0, yx: 0, yy: 100_000 },
              panToCenterPerZoom: { xx: 150_000, xy: 0, yx: 0, yy: 200_000 },
              sizePerZoom: { width: 300_000, height: 400_000 },
            },
          },
        },
      ],
    },
  ],
};

const mirroredInteractiveComposition: CompositionPlan = {
  frameBorder: { kind: "none" },
  sheets: [
    {
      ...interactiveComposition.sheets[0],
      frames: [
        {
          ...interactiveComposition.sheets[0].frames[0],
          photo: {
            ...interactiveComposition.sheets[0].frames[0].photo!,
            mirrorX: true,
          },
        },
      ],
    },
  ],
};

// The continuous Canvas fits the Sheet height into 500 - 2 * 28 = 444 screen
// px, so a 300 mm Sheet is drawn at 1.48 px/mm and a 222 mm Sheet at 2 px/mm.
// A pointer travel of d screen px therefore moves the Photo d / scale mm, and
// Pan is that travel divided by the slack between Photo and Frame (Pan 1 puts
// the Photo edge on the Frame edge).
test.each([
  {
    name: "a landscape Photo at 1.48 px/mm",
    compositionPlan: interactiveComposition,
    sheetHeightUm: 300_000,
    scale: 1.48,
    // 37 px = 25 mm of the 50 mm horizontal slack; there is no vertical slack.
    pointer: { x: 37, y: 20 },
    zoom: 1,
    expected: { x: 0.5, y: 0 },
  },
  {
    name: "a landscape Photo at 2 px/mm",
    compositionPlan: interactiveComposition,
    sheetHeightUm: 222_000,
    scale: 2,
    // 37 px = 18.5 mm of the 50 mm horizontal slack.
    pointer: { x: 37, y: 20 },
    zoom: 1,
    expected: { x: 0.37, y: 0 },
  },
  {
    name: "a portrait Photo at 1.48 px/mm",
    compositionPlan: portraitInteractiveComposition,
    sheetHeightUm: 300_000,
    scale: 1.48,
    // 74 px = 50 mm of the 100 mm vertical slack; there is no horizontal slack.
    pointer: { x: 20, y: 74 },
    zoom: 1,
    expected: { x: 0, y: 0.5 },
  },
  {
    name: "a portrait Photo at 2 px/mm",
    compositionPlan: portraitInteractiveComposition,
    sheetHeightUm: 222_000,
    scale: 2,
    // 74 px = 37 mm of the 100 mm vertical slack.
    pointer: { x: 20, y: 74 },
    zoom: 1,
    expected: { x: 0, y: 0.37 },
  },
  {
    // The 6000 x 4000 Photo turned 90 degrees covers 450 x 675 mm at Zoom 1.5:
    // 75 mm of horizontal and 237.5 mm of vertical slack. Its own X axis now
    // runs down the Sheet and its Y axis runs to the left.
    name: "a Photo rotated 90 degrees at 1.48 px/mm",
    compositionPlan: rotatedInteractiveComposition,
    sheetHeightUm: 300_000,
    scale: 1.48,
    // 37 px = 25 mm to the right, 74 px = 50 mm down.
    pointer: { x: 37, y: 74 },
    zoom: 1.5,
    expected: { x: 50 / 237.5, y: -25 / 75 },
  },
  {
    name: "a Photo rotated 90 degrees at 2 px/mm",
    compositionPlan: rotatedInteractiveComposition,
    sheetHeightUm: 222_000,
    scale: 2,
    // 37 px = 18.5 mm to the right, 74 px = 37 mm down.
    pointer: { x: 37, y: 74 },
    zoom: 1.5,
    expected: { x: 37 / 237.5, y: -18.5 / 75 },
  },
  {
    // The Core placement plan already carries the mirrored axes; the Canvas
    // must not flip the pointer travel a second time.
    name: "a mirrored landscape Photo at 1.48 px/mm",
    compositionPlan: mirroredInteractiveComposition,
    sheetHeightUm: 300_000,
    scale: 1.48,
    pointer: { x: 37, y: 20 },
    zoom: 1,
    expected: { x: 0.5, y: 0 },
  },
])(
  "converts the pointer travel of an unsaturated Pan into the exact normalized offset for $name",
  async ({ compositionPlan, sheetHeightUm, scale, pointer, zoom, expected }) => {
    const onTransformPreview = vi.fn();
    const onTransformCommit = vi.fn(
      async (_delta: PhotoTransformDelta) => true,
    );
    renderCanvas({
      compositionPlan: {
        ...compositionPlan,
        sheets: compositionPlan.sheets.map((sheet) => ({
          ...sheet,
          heightUm: sheetHeightUm,
        })),
      },
      onTransformPreview,
      onTransformCommit,
    });
    await finishPixiInitialization();
    expect(displayWithLabel("album-world").scale.x).toBeCloseTo(scale, 9);

    displayWithLabel("canvas-frame-frame-001").emit("pointerdown", {
      altKey: true,
      global: { x: 100, y: 100 },
      stopPropagation: vi.fn(),
    });
    pixiLifecycle.instances[0].stage.emit("globalpointermove", {
      global: { x: 100 + pointer.x, y: 100 + pointer.y },
    });

    expect(onTransformPreview).toHaveBeenLastCalledWith({
      frameId: "frame-001",
      panX: expect.closeTo(expected.x, 9),
      panY: expect.closeTo(expected.y, 9),
      zoom,
    });

    pixiLifecycle.instances[0].stage.emit("pointerup", {
      global: { x: 100 + pointer.x, y: 100 + pointer.y },
    });

    expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
      frameId: "frame-001",
      deltaPanX: expect.closeTo(expected.x, 9),
      deltaPanY: expect.closeTo(expected.y, 9),
      deltaZoom: 0,
    });
  },
);
