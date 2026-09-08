import { act, fireEvent } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { frameContentSwapCorpus } from "../test/frameContentSwapPreview";
import { displayWithLabel, finishPixiInitialization, getPixiLifecycle, renderCanvas, setupAlbumCanvasTestHarness } from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();

test.each(["release", "escape", "projection", "unmount", "zoom-pending"])("the ghost snapshots the visible crop once, follows the pointer, and cleans up on %s", async (end) => {
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
  expect(ghost.position).toMatchObject({ x: 318, y: 218 });
  expect(app.stage.children).toContain(ghost);
  const capturedPhoto = displayWithLabel("photo-drag-preview");
  expect(capturedPhoto.scale).toEqual(visiblePhoto.scale);
  expect(capturedPhoto.position).toEqual(visiblePhoto.position);
  fireEvent.pointerMove(window, { pointerId: 7, clientX: 420, clientY: 240 });
  expect(ghost.position).toMatchObject({ x: 438, y: 258 });
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
