import { act, render } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { PhotoTransformDelta } from "./AlbumCanvas";
import {
  interactiveComposition,
  pannedInteractiveComposition,
  rotatedInteractiveComposition,
} from "./albumCanvasTestFixtures";
import { createContinuousCanvasLayout } from "./canvasGeometry";
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

  const frame = displayWithHandler("pointerdown");
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

  const frame = displayWithHandler("pointerdown");
  const maskedViewport = frame.children.find(
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
    (child) =>
      Array.isArray((child as { children?: unknown[] }).children) &&
      (child as { children: unknown[] }).children.length > 0,
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

  const frame = displayWithHandler("pointerdown");
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

test("keeps every frame corner covered while panning a rotated photo", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: rotatedInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("pointerdown");
  const maskedViewport = frame.children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as { children: unknown[] };
  const photoLayer = maskedViewport.children.find(
    (child) =>
      Array.isArray((child as { children?: unknown[] }).children) &&
      (child as { children: unknown[] }).children.length > 0,
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

  const frame = displayWithHandler("pointerdown");
  const maskedViewport = frame.children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as { children: unknown[] };
  const photoLayer = maskedViewport.children.find(
    (child) =>
      Array.isArray((child as { children?: unknown[] }).children) &&
      (child as { children: unknown[] }).children.length > 0,
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
  const maskedDisplay = frame.children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as {
    children: unknown[];
    position: { x: number };
    scale: { y: number };
  };
  const photoLayer =
    (maskedDisplay.children.find(
      (child) =>
        Array.isArray((child as { children?: unknown[] }).children) &&
        (child as { children: unknown[] }).children.length > 0,
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

  const frame = displayWithHandler("pointerdown");
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
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 42 },
    onSelectFrame: vi.fn(),
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
