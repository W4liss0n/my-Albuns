import { act, fireEvent } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { frameContentSwapCorpus } from "../test/frameContentSwapPreview";
import { displayWithLabel, finishPixiInitialization, getPixiLifecycle, renderCanvas, setupAlbumCanvasTestHarness } from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();

test.each(["release", "escape", "projection", "unmount", "zoom-pending"])("the ghost shows the whole Photo near the pointer and cleans up on %s", async (end) => {
  vi.useFakeTimers();
  const composition = structuredClone(frameContentSwapCorpus.before.composition);
  const controls = { disabled: false, dragThreshold: { x: 5, y: 5 },
    resolveTarget: vi.fn(async () => ({ kind: "frame" as const, frameId: "swap-frame-1" })),
    commit: vi.fn(async () => true), onError: vi.fn() };
  const view = renderCanvas({ compositionPlan: composition, frameContentSwap: controls });
  await finishPixiInitialization();
  const lifecycle = getPixiLifecycle(), app = lifecycle.instances[0];
  vi.spyOn(app.canvas, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 1200, 500));
  app.canvas.setPointerCapture = vi.fn(); app.canvas.releasePointerCapture = vi.fn();
  if (end === "zoom-pending") act(() => displayWithLabel("canvas-frame-swap-frame-0").emit("wheel", {
    altKey: true, deltaY: -100, preventDefault: vi.fn(),
  }));
  const visiblePhoto = displayWithLabel("photo-pan-inside-preview");
  if (end === "zoom-pending") expect(visiblePhoto.scale.y).toBeGreaterThan(1);
  act(() => displayWithLabel("canvas-frame-swap-frame-0").emit("pointerdown", {
    button: 0, pointerId: 7, clientX: 100, clientY: 200, altKey: false,
    ctrlKey: false, metaKey: false, stopPropagation: vi.fn(),
  }));
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 103, clientY: 200 });
  expect(lifecycle.generatedTextures).toHaveLength(0);
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 300, clientY: 200 });
  await act(async () => { await Promise.resolve(); });
  expect(displayWithLabel("frame-content-drop-swap-frame-1").visible).toBe(true);
  const ghost = displayWithLabel("frame-content-drag-ghost");
  const texture = lifecycle.generatedTextures[0];
  expect(ghost.visible).toBe(true);
  expect(ghost.position).toMatchObject({ x: 306, y: 206 });
  expect(app.stage.children).toContain(ghost);
  const capturedPhoto = displayWithLabel("photo-drag-preview");
  expect(capturedPhoto.scale).toMatchObject({ x: 1, y: 1 });
  const bounds = texture.options.frame as { width: number; height: number };
  const resolution = texture.options.resolution as number;
  expect(bounds.width / bounds.height).toBeCloseTo(3 / 2);
  expect(bounds.width * resolution).toBeCloseTo(80);
  expect(bounds.height * resolution).toBeCloseTo(80 / 1.5);
  expect(capturedPhoto.position).toMatchObject({ x: bounds.width / 2, y: bounds.height / 2 });
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 420, clientY: 240 });
  expect(ghost.position).toMatchObject({ x: 426, y: 246 });
  expect(lifecycle.generatedTextures).toHaveLength(1);
  expect(texture.destroy).not.toHaveBeenCalled();
  if (end === "release") fireEvent.pointerUp(window, { pointerId: 7, clientX: 420, clientY: 240 });
  if (end === "escape" || end === "zoom-pending") fireEvent.keyDown(window, { key: "Escape" });
  if (end === "projection") view.rerenderCanvas({ composition: structuredClone(composition) });
  if (end === "unmount") view.unmount();
  expect(texture.destroy).toHaveBeenCalledExactlyOnceWith(true);
  expect(ghost.visible).toBe(false);
  expect(app.stage.children).not.toContain(ghost);
  if (end !== "release") expect(controls.commit).not.toHaveBeenCalled();
});

test.each([
  { frameIndex: 0, width: 600, height: 400, ghostWidth: 80, ghostHeight: 80 / 1.5 },
  { frameIndex: 1, width: 300, height: 700, ghostWidth: 60 * 3 / 7, ghostHeight: 60 },
])("the ghost uses the full cached Photo $width x $height without Frame transforms", async ({ frameIndex, width, height, ghostWidth, ghostHeight }) => {
  const composition = structuredClone(frameContentSwapCorpus.before.composition);
  const frame = composition.sheets[0].frames[frameIndex];
  const sourcePhoto = frame.photo!;
  sourcePhoto.mirrorX = true;
  sourcePhoto.rotationDegrees = 180;
  const sourceTexture = { orig: { width, height }, destroy: vi.fn() };
  const controls = { disabled: false, dragThreshold: { x: 5, y: 5 },
    resolveTarget: vi.fn(async () => ({ kind: "invalid" as const })), commit: vi.fn(async () => true), onError: vi.fn() };
  renderCanvas({ compositionPlan: composition, frameContentSwap: controls,
    mediaPreviewUrls: { [sourcePhoto.mediaId]: "http://myalbuns-cache.localhost/full-photo" } });
  await finishPixiInitialization();
  const lifecycle = getPixiLifecycle(), app = lifecycle.instances[0];
  await act(async () => { lifecycle.resolveAssetLoads[0](sourceTexture); await Promise.resolve(); });
  vi.spyOn(app.canvas, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, 1200, 500));
  app.canvas.setPointerCapture = vi.fn(); app.canvas.releasePointerCapture = vi.fn();
  const source = [...lifecycle.displays].reverse().find((node) => node.label === `canvas-frame-${frame.frameId}` && node.handlers.has("pointerdown"))!;
  const sourceSpriteCount = lifecycle.spriteTextures.filter((texture) => texture === sourceTexture).length;
  act(() => source.emit("pointerdown", { button: 0, pointerId: 7, clientX: 100, clientY: 200,
    altKey: false, ctrlKey: false, metaKey: false, stopPropagation: vi.fn() }));
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 300, clientY: 200 });
  await act(async () => { await Promise.resolve(); });
  const snapshot = lifecycle.generatedTextures[0].options;
  expect(snapshot.frame).toMatchObject({ x: 0, y: 0, width, height });
  expect(snapshot.target).toMatchObject({ rotation: 0, scale: { x: 1, y: 1 }, mask: null,
    position: { x: width / 2, y: height / 2 } });
  expect(lifecycle.spriteTextures.filter((texture) => texture === sourceTexture)).toHaveLength(sourceSpriteCount + 1);
  expect(width * (snapshot.resolution as number)).toBeCloseTo(ghostWidth);
  expect(height * (snapshot.resolution as number)).toBeCloseTo(ghostHeight);
  fireEvent.keyDown(window, { key: "Escape" });
  expect(sourceTexture.destroy).not.toHaveBeenCalled();
  expect(lifecycle.generatedTextures[0].destroy).toHaveBeenCalledExactlyOnceWith(true);
});
