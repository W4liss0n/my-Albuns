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
  landscapePhotoPlacement,
  pannedInteractiveComposition,
  rotatedInteractiveComposition,
  twoFrameInteractiveComposition,
  withPhotoPlacement,
} from "./albumCanvasTestFixtures";
import { createContinuousCanvasLayout } from "./canvasGeometry";

setupAlbumCanvasTestHarness();
const pixiLifecycle = getPixiLifecycle();

type PixiDisplay = (typeof pixiLifecycle.displays)[number];

// A Sheet rebuild replaces every Frame node; only the live one has handlers.
function liveFrame(frameId: string) {
  const frame = [...pixiLifecycle.displays].reverse().find(
    (display) =>
      display.label === `canvas-frame-${frameId}` &&
      display.handlers.has("pointerdown"),
  );
  if (!frame) throw new Error(`Live Frame ${frameId} not found`);
  return frame;
}

function livePhotoLayers(frameId: string) {
  const frame = liveFrame(frameId);
  const child = (parent: PixiDisplay, label: string) =>
    (parent.children as PixiDisplay[]).find(
      (display) => display.label === label,
    )!;
  const viewport = (
    child(frame, `frame-content-${frameId}`).children as PixiDisplay[]
  ).find((display) => display.mask)!;
  return {
    frame,
    inside: child(viewport, "photo-pan-inside-preview"),
    outside: child(frame, "photo-pan-outside-preview"),
    thirds: child(frame, "photo-pan-thirds-guides"),
  };
}

function altPress(frameId: string, x = 0, y = 0) {
  liveFrame(frameId).emit("pointerdown", {
    altKey: true,
    global: { x, y },
    stopPropagation: vi.fn(),
  });
}

function altWheel(frameId: string, deltaY = -100) {
  liveFrame(frameId).emit("wheel", {
    altKey: true,
    deltaY,
    preventDefault: vi.fn(),
  });
}

/** Commits the test settles itself, so it owns when the Project answers. */
function deferredTransformCommits() {
  const answers: Array<(accepted: boolean) => void> = [];
  const onTransformCommit = vi.fn(
    (_delta: PhotoTransformDelta) =>
      new Promise<boolean>((resolve) => answers.push(resolve)),
  );
  // Settles the oldest commit and runs its continuations, without a render.
  const answer = (accepted: boolean) =>
    act(async () => {
      answers.shift()!(accepted);
      await Promise.resolve();
    });
  return { onTransformCommit, answer };
}

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

test("keeps the hovered Sheet Bar shown when a committed Alt-Pan redraws the Sheet", async () => {
  vi.useFakeTimers();
  const view = renderCanvas({ compositionPlan: interactiveComposition });
  await finishPixiInitialization();
  const liveDisplay = (label: string) =>
    [...pixiLifecycle.displays].reverse().find(
      (display) => display.label === label,
    )!;
  const hoveredBar = liveDisplay("sheet-bar-sheet-001");
  liveDisplay("canvas-sheet-sheet-001").emit("pointerenter", {});
  await act(async () => {
    await vi.advanceTimersByTimeAsync(160);
  });
  expect(hoveredBar.alpha).toBe(0.55);

  altPress("frame-001");
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: -45, y: 0 },
  });
  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: -45, y: 0 },
  });
  // The committed Pan comes back as a new Sheet, drawn under a still pointer.
  view.rerenderCanvas({ composition: pannedInteractiveComposition });

  const redrawnBar = liveDisplay("sheet-bar-sheet-001");
  expect(redrawnBar).not.toBe(hoveredBar);
  expect(redrawnBar.alpha).toBe(0.55);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(160);
  });
  expect(redrawnBar.alpha).toBe(0.55);

  liveDisplay("canvas-sheet-sheet-001").emit("pointerleave", {});
  await act(async () => {
    await vi.advanceTimersByTimeAsync(160);
  });
  expect(redrawnBar.alpha).toBe(0);
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

  // The Zoom keeps the Photo point at the Frame center (x 150 mm).
  const zoomedOffset = (pannedX - 150) * 1.06;
  expect(photoLayer.scale.y).toBeGreaterThan(1);
  expect(photoLayer.position.x).toBeCloseTo(150 + zoomedOffset, 4);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).not.toHaveBeenCalled();

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 40, y: 0 },
  });

  // The centered Photo stays centered after the Core's anchored Zoom, so the
  // whole offset is Pan over the 62 mm of room at Zoom 1.06.
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: expect.closeTo(zoomedOffset / 62, 9),
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.06, 6),
  });
});

// The panned fixture puts the Photo center 45 mm left of the Frame center,
// at Pan -0.9 of 50 mm of room. Zoom 1.06 leaves 62 mm of room.
test("continues an Alt-Pan from where a mid-drag wheel Zoom moved the Photo", async () => {
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

  const frame = displayWithLabel("canvas-frame-frame-001");
  const photoLayer = displayWithLabel("photo-pan-inside-preview");
  const stage = pixiLifecycle.instances[0].stage;
  const step = 20 / 1.48;

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  stage.emit("globalpointermove", { global: { x: 20, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(105 + step, 4);

  frame.emit("wheel", {
    altKey: true,
    deltaY: -100,
    preventDefault: vi.fn(),
  });

  const zoomedOffset = (step - 45) * 1.06;
  expect(photoLayer.position.x).toBeCloseTo(150 + zoomedOffset, 4);
  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: expect.closeTo(zoomedOffset / 62, 9),
    panY: 0,
    zoom: expect.closeTo(1.06, 9),
  });

  stage.emit("globalpointermove", { global: { x: 20, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(150 + zoomedOffset, 4);

  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(150 + zoomedOffset + step, 4);

  stage.emit("pointerup", { global: { x: 40, y: 0 } });

  // The Core first zooms the committed Pan around the Frame center (offset
  // -45 * 1.06 mm), then adds the Pan the drag made on top of it.
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: expect.closeTo((zoomedOffset + step + 45 * 1.06) / 62, 9),
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.06, 6),
  });
});

test("keeps the Photo still after a wheel Zoom during an Alt-Pan dragged past the limit", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  const photoLayer = displayWithLabel("photo-pan-inside-preview");
  const stage = pixiLifecycle.instances[0].stage;
  const step = 10 / 1.48;

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  // The pointer asks for 37 mm, but the Pan stops at -1 (center 100 mm).
  stage.emit("globalpointermove", { global: { x: -100, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(100, 4);

  frame.emit("wheel", {
    altKey: true,
    deltaY: -100,
    preventDefault: vi.fn(),
  });
  expect(photoLayer.position.x).toBeCloseTo(150 - 50 * 1.06, 4);

  stage.emit("globalpointermove", { global: { x: -100, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(150 - 50 * 1.06, 4);

  stage.emit("globalpointermove", { global: { x: -110, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(150 - 50 * 1.06 - step, 4);

  stage.emit("pointerup", { global: { x: -110, y: 0 } });

  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: expect.closeTo((-50 * 1.06 - step + 45 * 1.06) / 62, 9),
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.06, 6),
  });
});

// At Zoom 1 the landscape Photo has no vertical room, so any vertical drift of
// the pointer is dragged past the limit; Zoom 1.06 opens 6 mm of room.
test("keeps the Photo still on the axis without room after a mid-drag wheel Zoom", async () => {
  vi.useFakeTimers();
  renderCanvas({ compositionPlan: pannedInteractiveComposition });
  await finishPixiInitialization();

  const photoLayer = livePhotoLayers("frame-001").inside;
  const stage = pixiLifecycle.instances[0].stage;
  altPress("frame-001");
  stage.emit("globalpointermove", { global: { x: 0, y: 60 } });
  expect(photoLayer.position.x).toBeCloseTo(105, 9);
  expect(photoLayer.position.y).toBeCloseTo(100, 9);

  altWheel("frame-001");
  expect(photoLayer.position.x).toBeCloseTo(150 - 45 * 1.06, 9);
  expect(photoLayer.position.y).toBeCloseTo(100, 9);

  stage.emit("globalpointermove", { global: { x: 1, y: 60 } });
  expect(photoLayer.position.x).toBeCloseTo(150 - 45 * 1.06 + 1 / 1.48, 9);
  expect(photoLayer.position.y).toBeCloseTo(100, 9);
});

test("commits a past-the-limit Alt-Pan where a mid-drag wheel Zoom left it", async () => {
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

  const photoLayer = livePhotoLayers("frame-001").inside;
  const stage = pixiLifecycle.instances[0].stage;
  altPress("frame-001");
  stage.emit("globalpointermove", { global: { x: -100, y: 0 } });
  altWheel("frame-001");
  // Pan -1 at Zoom 1 (offset -50 mm) zoomed around the Frame center.
  const shownPanX = (-50 * 1.06) / 62;
  expect(photoLayer.position.x).toBeCloseTo(150 - 50 * 1.06, 9);
  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: expect.closeTo(shownPanX, 9),
    panY: 0,
    zoom: expect.closeTo(1.06, 9),
  });

  stage.emit("pointerup", { global: { x: -100, y: 0 } });

  expect(photoLayer.position.x).toBeCloseTo(150 - 50 * 1.06, 9);
  // The Core zooms the committed Pan -0.9 around the Frame center first.
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: expect.closeTo(shownPanX - (-45 * 1.06) / 62, 9),
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.06, 6),
  });
});

// Zooming out at the Pan limit pulls the Photo in; zooming back in keeps it
// there, so the drag must continue from that point, not from the limit.
test("keeps the Photo still after a wheel Zoom out and back in during an Alt-Pan", async () => {
  vi.useFakeTimers();
  renderCanvas({ compositionPlan: pannedInteractiveComposition });
  await finishPixiInitialization();

  const photoLayer = livePhotoLayers("frame-001").inside;
  const stage = pixiLifecycle.instances[0].stage;
  altWheel("frame-001");
  altWheel("frame-001");
  altWheel("frame-001");
  expect(photoLayer.position.x).toBeCloseTo(150 - 45 * 1.18, 9);

  altPress("frame-001");
  // Zoom 1.18 leaves 86 mm of room: drag the 53.1 mm offset to the limit.
  const toLimit = -(86 - 45 * 1.18) * 1.48;
  stage.emit("globalpointermove", { global: { x: toLimit, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(150 - 86, 9);

  altWheel("frame-001", 100);
  // Zoom 1.12 leaves 74 mm of room.
  expect(photoLayer.position.x).toBeCloseTo(150 - 74, 9);
  altWheel("frame-001", -100);
  const shownX = 150 - (74 * 1.18) / 1.12;
  expect(photoLayer.position.x).toBeCloseTo(shownX, 9);

  stage.emit("globalpointermove", { global: { x: toLimit - 1, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(shownX - 1 / 1.48, 9);
});

test("starts an Alt-Pan from the anchored preview of a settling wheel Zoom", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  const photoLayer = displayWithLabel("photo-pan-inside-preview");
  frame.emit("wheel", {
    altKey: true,
    deltaY: -100,
    preventDefault: vi.fn(),
  });
  expect(photoLayer.position.x).toBeCloseTo(150 - 45 * 1.06, 4);

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 0, y: 0 },
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });

  expect(photoLayer.position.x).toBeCloseTo(150 - 45 * 1.06, 4);
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: expect.closeTo(0, 9),
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.06, 6),
  });
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
  // The Photo point at the Frame center stays put: the -45 mm offset grows
  // to -47.7 mm, which is Pan -47.7 / 62 at Zoom 1.06.
  expect(photoLayer.scale.y).toBeGreaterThan(1);
  expect(photoLayer.position.x).toBeCloseTo(150 - 45 * 1.06, 4);
  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: expect.closeTo((-45 * 1.06) / 62, 9),
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

test("previews the contextual panel Zoom around the Frame center", async () => {
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
  });
  await finishPixiInitialization();
  const photoLayer = displayWithLabel("photo-pan-inside-preview");
  expect(photoLayer.position.x).toBeCloseTo(105, 4);

  view.rerenderCanvas({
    photoZoomPreview: { frameId: "frame-001", value: 1.12 },
  });
  expect(photoLayer.position.x).toBeCloseTo(150 - 45 * 1.12, 4);

  view.rerenderCanvas({ photoZoomPreview: null });
  expect(photoLayer.position.x).toBeCloseTo(105, 4);
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

test("Alt-Pan selects the Frame it moves, so later shortcuts reach it", async () => {
  const onTransformCommit = vi.fn(async (_delta: PhotoTransformDelta) => true);
  const onSelectFrame = vi.fn();
  const onFocusSheet = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformCommit,
    onSelectFrame,
    onFocusSheet,
  });
  await finishPixiInitialization();

  const frame = displayWithLabel("canvas-frame-frame-001");
  frame.emit("pointerdown", { altKey: true, global: { x: 0, y: 0 }, stopPropagation: vi.fn() });
  expect(onSelectFrame).toHaveBeenCalledExactlyOnceWith("frame-001");
  expect(onFocusSheet).toHaveBeenCalledWith("sheet-001");

  const stage = pixiLifecycle.instances[0].stage;
  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });
  stage.emit("pointerup", { global: { x: 40, y: 0 } });
  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(onTransformCommit.mock.calls[0][0].deltaPanX).toBeGreaterThan(0);
});

test("Alt-Pan on the selected Frame selects it again, so the contextual panel returns to it", async () => {
  const onSelectFrame = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    selectedFrameIds: ["frame-001"],
    onSelectFrame,
  });
  await finishPixiInitialization();

  displayWithLabel("canvas-frame-frame-001").emit("pointerdown", {
    altKey: true, global: { x: 0, y: 0 }, stopPropagation: vi.fn(),
  });
  expect(onSelectFrame).toHaveBeenCalledExactlyOnceWith("frame-001");
  pixiLifecycle.instances[0].stage.emit("pointerup", { global: { x: 0, y: 0 } });
});

// React draws a committed projection in a later task than the one that
// settles the commit. Until then the Photo node keeps the placement from
// before the commit, so a gesture started there would jump back to it.
// The panned Photo zoomed to 1.18 around the Frame center sits at
// 150 - 45 * 1.18 = 96.9 mm.
const zoomedPannedComposition = withPhotoPlacement(
  pannedInteractiveComposition,
  "frame-001",
  landscapePhotoPlacement(1.18, -45_000 * 1.18),
);

test("ignores a wheel Zoom on a Photo until the scene draws its committed Zoom", async () => {
  vi.useFakeTimers();
  const { onTransformCommit, answer } = deferredTransformCommits();
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  view.rerenderCanvas({ revision: 1 });

  altWheel("frame-001");
  altWheel("frame-001");
  altWheel("frame-001");
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.18, 6),
  });
  await answer(true);

  const stale = livePhotoLayers("frame-001").inside;
  expect(stale.scale.y).toBeCloseTo(1.18, 9);
  expect(stale.position.x).toBeCloseTo(96.9, 9);
  onTransformPreview.mockClear();
  altWheel("frame-001");
  expect(stale.scale.y).toBeCloseTo(1.18, 9);
  expect(stale.position.x).toBeCloseTo(96.9, 9);
  expect(onTransformPreview).not.toHaveBeenCalled();

  view.rerenderCanvas({ composition: zoomedPannedComposition, revision: 2 });
  altWheel("frame-001");
  const drawn = livePhotoLayers("frame-001").inside;
  expect(drawn).not.toBe(stale);
  expect(drawn.scale.y).toBeCloseTo(1.24 / 1.18, 9);
  expect(drawn.position.x).toBeCloseTo(150 - 45 * 1.24, 9);
  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: expect.closeTo((-45 * 1.24) / 98, 9),
    panY: 0,
    zoom: expect.closeTo(1.24, 9),
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).toHaveBeenCalledTimes(2);
  expect(onTransformCommit).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.06, 6),
  });
});

test("starts no Alt-Pan on a Photo until the scene draws its committed Zoom", async () => {
  vi.useFakeTimers();
  const { onTransformCommit, answer } = deferredTransformCommits();
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  view.rerenderCanvas({ revision: 1 });
  altWheel("frame-001");
  altWheel("frame-001");
  altWheel("frame-001");
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  await answer(true);

  onTransformPreview.mockClear();
  altPress("frame-001");
  expect(livePhotoLayers("frame-001").outside.visible).toBe(false);
  // Selecting the Frame flushes the pending projection render at once.
  view.rerenderCanvas({ composition: zoomedPannedComposition, revision: 2 });
  const stage = pixiLifecycle.instances[0].stage;
  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });
  stage.emit("pointerup", { global: { x: 40, y: 0 } });

  const drawn = livePhotoLayers("frame-001");
  expect(drawn.inside.position.x).toBeCloseTo(96.9, 9);
  expect(drawn.inside.scale.y).toBe(1);
  expect(drawn.outside.visible).toBe(false);
  expect(drawn.thirds.visible).toBe(false);
  expect(onTransformPreview).not.toHaveBeenCalled();
  expect(onTransformCommit).toHaveBeenCalledOnce();
});

test("frees a Photo when its accepted commit leaves the composition unchanged", async () => {
  vi.useFakeTimers();
  const { onTransformCommit, answer } = deferredTransformCommits();
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  view.rerenderCanvas({ revision: 1 });
  altWheel("frame-001");
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  await answer(true);
  const photo = livePhotoLayers("frame-001").inside;
  expect(photo.scale.y).toBeCloseTo(1.06, 9);

  view.rerenderCanvas({ revision: 2 });
  // The drawn revision holds the Zoom from before the commit, and so does the
  // Photo; an Alt-click that does not move it changes nothing.
  expect(livePhotoLayers("frame-001").inside).toBe(photo);
  expect(photo.scale.y).toBe(1);
  expect(photo.position.x).toBeCloseTo(105, 9);
  altPress("frame-001");
  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 0, y: 0 },
  });
  expect(onTransformCommit).toHaveBeenCalledOnce();

  onTransformPreview.mockClear();
  altWheel("frame-001");
  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: expect.closeTo((-45 * 1.06) / 62, 9),
    panY: 0,
    zoom: expect.closeTo(1.06, 9),
  });
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).toHaveBeenCalledTimes(2);
});

// A rebuild before the projection is drawn comes from the Project as it was
// before the commit: the commit's preview moves to the new node and the Frame
// stays busy, instead of showing the old Zoom and starting a gesture there.
test("keeps a committed Zoom on screen when another Photo's texture loads before it is drawn", async () => {
  vi.useFakeTimers();
  const { onTransformCommit, answer } = deferredTransformCommits();
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    mediaPreviewUrls: {
      "media-002": "http://myalbuns-cache.localhost/media-002.jpg",
    },
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  view.rerenderCanvas({ revision: 1 });
  altWheel("frame-001");
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).toHaveBeenCalledOnce();

  const committing = livePhotoLayers("frame-001");
  await act(async () => {
    pixiLifecycle.resolveAssetLoads[0]({ label: "media-002" });
    await Promise.resolve();
  });
  const rebuilt = livePhotoLayers("frame-001");
  expect(rebuilt.frame).not.toBe(committing.frame);
  expect(rebuilt.inside.scale.y).toBeCloseTo(1.06, 9);

  await answer(true);
  onTransformPreview.mockClear();
  altWheel("frame-001");
  expect(rebuilt.inside.scale.y).toBeCloseTo(1.06, 9);
  expect(onTransformPreview).not.toHaveBeenCalled();
});

test("rolls a rejected Zoom back on the node rebuilt while it was committing", async () => {
  vi.useFakeTimers();
  const { onTransformCommit, answer } = deferredTransformCommits();
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    mediaPreviewUrls: {
      "media-002": "http://myalbuns-cache.localhost/media-002.jpg",
    },
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  view.rerenderCanvas({ revision: 1 });
  altWheel("frame-001");
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  await act(async () => {
    pixiLifecycle.resolveAssetLoads[0]({ label: "media-002" });
    await Promise.resolve();
  });

  await answer(false);
  const rebuilt = livePhotoLayers("frame-001").inside;
  expect(rebuilt.scale.y).toBe(1);
  expect(rebuilt.position.x).toBeCloseTo(150, 9);
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);

  altWheel("frame-001");
  expect(rebuilt.scale.y).toBeCloseTo(1.06, 9);
});

// A rebuilt Sheet replaces every Photo node on it. A commit on another Frame or
// a preview texture finishing its load leaves the dragged Photo's placement
// unchanged, so its gesture moves to the new node where it was.
const otherFrameCommitted = withPhotoPlacement(
  twoFrameInteractiveComposition,
  "frame-001",
  landscapePhotoPlacement(1, 25_000),
);

test("keeps an Alt-Pan when another Frame's commit rebuilds the Sheet", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const view = renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();
  const stage = pixiLifecycle.instances[0].stage;
  const step = 20 / 1.48;

  const dragged = livePhotoLayers("frame-002");
  altPress("frame-002");
  stage.emit("globalpointermove", { global: { x: 20, y: 0 } });
  expect(dragged.inside.position.x).toBeCloseTo(150 + step, 9);

  view.rerenderCanvas({ composition: otherFrameCommitted });

  const rebuilt = livePhotoLayers("frame-002");
  expect(rebuilt.frame).not.toBe(dragged.frame);
  expect(rebuilt.inside.position.x).toBeCloseTo(150 + step, 9);
  expect(rebuilt.outside.position.x).toBeCloseTo(150 + step, 9);
  expect(rebuilt.outside.visible).toBe(true);
  expect(rebuilt.thirds.visible).toBe(true);

  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });
  expect(rebuilt.inside.position.x).toBeCloseTo(150 + 2 * step, 9);
  stage.emit("pointerup", { global: { x: 40, y: 0 } });

  expect(rebuilt.outside.visible).toBe(false);
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-002",
    deltaPanX: expect.closeTo((2 * step) / 50, 9),
    deltaPanY: 0,
    deltaZoom: 0,
  });
});

test("keeps a wheel Zoom when another Frame's commit rebuilds the Sheet", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const view = renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  altWheel("frame-002");
  expect(livePhotoLayers("frame-002").inside.scale.y).toBeCloseTo(1.06, 9);
  view.rerenderCanvas({ composition: otherFrameCommitted });

  expect(livePhotoLayers("frame-002").inside.scale.y).toBeCloseTo(1.06, 9);
  altWheel("frame-002");
  expect(livePhotoLayers("frame-002").inside.scale.y).toBeCloseTo(1.12, 9);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-002",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.12, 6),
  });
});

test("keeps an Alt-Pan when another Photo's preview texture loads", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    mediaPreviewUrls: {
      "media-001": "http://myalbuns-cache.localhost/media-001.jpg",
    },
    onTransformCommit,
  });
  await finishPixiInitialization();
  const stage = pixiLifecycle.instances[0].stage;
  const step = 20 / 1.48;

  const dragged = livePhotoLayers("frame-002");
  altPress("frame-002");
  stage.emit("globalpointermove", { global: { x: 20, y: 0 } });
  await act(async () => {
    pixiLifecycle.resolveAssetLoads[0]({ label: "media-001" });
    await Promise.resolve();
  });

  const rebuilt = livePhotoLayers("frame-002");
  expect(rebuilt.frame).not.toBe(dragged.frame);
  expect(rebuilt.inside.position.x).toBeCloseTo(150 + step, 9);
  expect(rebuilt.thirds.visible).toBe(true);
  stage.emit("pointerup", { global: { x: 20, y: 0 } });
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-002",
    deltaPanX: expect.closeTo(step / 50, 9),
    deltaPanY: 0,
    deltaZoom: 0,
  });
});

// An Undo or another change to the dragged Photo itself: the drag no longer
// describes what the Project holds, so it ends without a commit.
const draggedFrameChanged = withPhotoPlacement(
  twoFrameInteractiveComposition,
  "frame-002",
  landscapePhotoPlacement(1, -25_000),
);

test("ends an Alt-Pan when the dragged Photo's placement changes under it", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  const stage = pixiLifecycle.instances[0].stage;

  altPress("frame-002");
  stage.emit("globalpointermove", { global: { x: 20, y: 0 } });
  view.rerenderCanvas({ composition: draggedFrameChanged });

  expect(onTransformPreview).toHaveBeenLastCalledWith(null);
  const rebuilt = livePhotoLayers("frame-002");
  expect(rebuilt.inside.position.x).toBeCloseTo(125, 9);
  expect(rebuilt.outside.visible).toBe(false);
  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });
  stage.emit("pointerup", { global: { x: 40, y: 0 } });
  expect(rebuilt.inside.position.x).toBeCloseTo(125, 9);
  expect(onTransformCommit).not.toHaveBeenCalled();
});

test("ends a wheel Zoom when the zoomed Photo's placement changes under it", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();

  altWheel("frame-002");
  view.rerenderCanvas({ composition: draggedFrameChanged });
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);
  expect(livePhotoLayers("frame-002").inside.scale.y).toBe(1);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).not.toHaveBeenCalled();
});

test("zooms the dragged Photo when Alt+wheel lands on another Frame during an Alt-Pan", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  renderCanvas({
    compositionPlan: twoFrameInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();
  const stage = pixiLifecycle.instances[0].stage;
  const step = 20 / 1.48;

  altPress("frame-002");
  stage.emit("globalpointermove", { global: { x: 20, y: 0 } });
  altWheel("frame-001");

  const dragged = livePhotoLayers("frame-002").inside;
  expect(dragged.scale.y).toBeCloseTo(1.06, 9);
  expect(dragged.position.x).toBeCloseTo(150 + step * 1.06, 9);
  expect(livePhotoLayers("frame-001").inside.scale.y).toBe(1);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).not.toHaveBeenCalled();

  stage.emit("pointerup", { global: { x: 20, y: 0 } });
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-002",
    deltaPanX: expect.closeTo((step * 1.06) / 62, 9),
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.06, 6),
  });
});

// The contextual panel keeps its Zoom draft as the Canvas preview until its
// own commit is drawn; a canvas gesture on the same Photo would fight it.
test("starts no canvas gesture on a Photo while the contextual panel previews its Zoom", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  view.rerenderCanvas({
    photoZoomPreview: { frameId: "frame-001", value: 1.12 },
  });
  const photo = livePhotoLayers("frame-001");
  expect(photo.inside.position.x).toBeCloseTo(150 - 45 * 1.12, 9);

  const stage = pixiLifecycle.instances[0].stage;
  altPress("frame-001");
  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });
  stage.emit("pointerup", { global: { x: 40, y: 0 } });
  altWheel("frame-001");
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });

  expect(photo.inside.position.x).toBeCloseTo(150 - 45 * 1.12, 9);
  expect(photo.inside.scale.y).toBeCloseTo(1.12, 9);
  expect(photo.outside.visible).toBe(false);
  expect(onTransformPreview).not.toHaveBeenCalled();
  expect(onTransformCommit).not.toHaveBeenCalled();
});

test("keeps an Alt-Pan over a contextual panel Zoom preview that opens during it", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();
  const photoLayer = livePhotoLayers("frame-001").inside;
  const stage = pixiLifecycle.instances[0].stage;
  const step = 20 / 1.48;

  altPress("frame-001");
  stage.emit("globalpointermove", { global: { x: 20, y: 0 } });
  view.rerenderCanvas({
    photoZoomPreview: { frameId: "frame-001", value: 1.12 },
  });
  expect(photoLayer.position.x).toBeCloseTo(105 + step, 9);
  expect(photoLayer.scale.y).toBe(1);

  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });
  expect(photoLayer.position.x).toBeCloseTo(105 + 2 * step, 9);
  stage.emit("pointerup", { global: { x: 40, y: 0 } });
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: expect.closeTo((2 * step) / 50, 9),
    deltaPanY: 0,
    deltaZoom: 0,
  });
});

// A mode change rebuilds every Photo node. A wheel Zoom waiting for its
// settle is already let go of, so it is committed and stays on screen; an
// Alt-Pan still held is cancelled.
test.each([
  { name: "Sheet editing", mode: { kind: "sheet-editing", sheetId: "sheet-001" } },
  { name: "the layout panel", mode: { kind: "normal", isolatedSheetId: "sheet-001" } },
] as const)("commits a settling wheel Zoom when $name opens", async ({ mode }) => {
  vi.useFakeTimers();
  const { onTransformCommit, answer } = deferredTransformCommits();
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  view.rerenderCanvas({ revision: 1 });
  altWheel("frame-001");
  altWheel("frame-001");
  const zoomed = livePhotoLayers("frame-001");

  view.rerenderCanvas({ mode, revision: 1 });
  expect(onTransformCommit).toHaveBeenCalledExactlyOnceWith({
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: expect.closeTo(0.12, 6),
  });
  const rebuilt = livePhotoLayers("frame-001");
  expect(rebuilt.frame).not.toBe(zoomed.frame);
  expect(rebuilt.inside.scale.y).toBeCloseTo(1.12, 9);
  expect(rebuilt.inside.position.x).toBeCloseTo(150 - 45 * 1.12, 9);
  expect(onTransformPreview).not.toHaveBeenLastCalledWith(null);

  await answer(true);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(rebuilt.inside.scale.y).toBeCloseTo(1.12, 9);
});

test("cancels a held Alt-Pan when Sheet editing opens", async () => {
  const onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  );
  const onTransformPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onTransformCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();
  const stage = pixiLifecycle.instances[0].stage;
  altPress("frame-001");
  stage.emit("globalpointermove", { global: { x: 40, y: 0 } });

  view.rerenderCanvas({ mode: { kind: "sheet-editing", sheetId: "sheet-001" } });
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);
  const rebuilt = livePhotoLayers("frame-001");
  expect(rebuilt.inside.position.x).toBeCloseTo(105, 9);
  expect(rebuilt.outside.visible).toBe(false);

  stage.emit("globalpointermove", { global: { x: 60, y: 0 } });
  stage.emit("pointerup", { global: { x: 60, y: 0 } });
  expect(rebuilt.inside.position.x).toBeCloseTo(105, 9);
  expect(onTransformCommit).not.toHaveBeenCalled();
});
