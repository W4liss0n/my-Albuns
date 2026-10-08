import { act, fireEvent, renderHook } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { displayWithLabel, finishPixiInitialization, getPixiLifecycle, renderCanvas, setupAlbumCanvasTestHarness } from "./albumCanvasTestHarness";
import { interactiveComposition } from "./albumCanvasTestFixtures";
import type { AlbumCanvasMode } from "./albumCanvasContract";
import { useCanvasModeKeyboardShortcuts } from "../editor/useCanvasModeKeyboardShortcuts";
import { frameContentSwapCorpus } from "../../test/frameContentSwapPreview";
import { frameGeometryPreview } from "../../test/frameGeometryPreview";

setupAlbumCanvasTestHarness();

const editing: AlbumCanvasMode = { kind: "sheet-editing", sheetId: "sheet-001" };

interface ActiveGesture {
  canvas: HTMLCanvasElement;
  active(): boolean;
  release(): void;
  changedContent(): boolean;
}

function idleFrameGeometry() {
  return { disabled: false, dragThreshold: { x: 5, y: 5 },
    preview: vi.fn(async () => frameGeometryPreview([interactiveComposition.sheets[0].frames[0]])),
    commit: vi.fn(async () => null), onError: vi.fn() };
}

async function preparedCanvas() {
  await finishPixiInitialization();
  const app = getPixiLifecycle().instances[0];
  vi.spyOn(app.canvas, "getBoundingClientRect").mockReturnValue(new DOMRect(0, 0, app.screen.width, app.screen.height));
  app.canvas.setPointerCapture = vi.fn();
  app.canvas.releasePointerCapture = vi.fn();
  return app;
}

const gestures: Record<string, { mode: AlbumCanvasMode; begin(): Promise<ActiveGesture> }> = {
  "a Frame drag": { mode: editing, begin: async () => {
    const frameGeometry = idleFrameGeometry();
    renderCanvas({ mode: editing, compositionPlan: interactiveComposition, selectedFrameIds: ["frame-001"], frameGeometry });
    const app = await preparedCanvas();
    const frame = displayWithLabel("canvas-frame-frame-001");
    act(() => frame.emit("pointerdown", { button: 0, pointerId: 7, clientX: 100, clientY: 100,
      altKey: false, shiftKey: false, stopPropagation: vi.fn(), currentTarget: frame }));
    fireEvent.pointerMove(window, { pointerId: 7, clientX: 140, clientY: 120 });
    return { canvas: app.canvas,
      active: () => app.canvas.classList.contains("pixi-canvas--frame-gesture"),
      release: () => fireEvent.pointerUp(window, { pointerId: 7, clientX: 150, clientY: 130 }),
      changedContent: () => frameGeometry.commit.mock.calls.length > 0 };
  } },
  "an area selection": { mode: editing, begin: async () => {
    const view = renderCanvas({ mode: editing, compositionPlan: interactiveComposition, frameGeometry: idleFrameGeometry() });
    await finishPixiInitialization();
    const onSelectFrames = vi.fn();
    view.rerenderCanvas({ onSelectFrames });
    const app = await preparedCanvas();
    const point = (x: number, y: number) => {
      const world = displayWithLabel("album-world");
      const sheet = displayWithLabel("canvas-sheet-sheet-001");
      return { clientX: world.position.x + (sheet.position.x + x) * world.scale.x,
        clientY: world.position.y + (sheet.position.y + y) * world.scale.y };
    };
    act(() => app.stage.emit("pointerdown", { target: app.stage, button: 0, pointerId: 7, ...point(0, 0),
      ctrlKey: false, stopPropagation: vi.fn() }));
    fireEvent.pointerMove(window, { pointerId: 7, ...point(200, 150) });
    return { canvas: app.canvas,
      active: () => displayWithLabel("frame-area-selection").visible,
      release: () => fireEvent.pointerUp(window, { pointerId: 7, ...point(200, 150) }),
      changedContent: () => onSelectFrames.mock.calls.length > 0 };
  } },
  "a camera Pan": { mode: editing, begin: async () => {
    renderCanvas({ mode: editing, compositionPlan: interactiveComposition, frameGeometry: idleFrameGeometry() });
    const app = await preparedCanvas();
    fireEvent.wheel(app.canvas, { ctrlKey: true, deltaY: -500, clientX: 600, clientY: 250 });
    const world = displayWithLabel("album-world");
    const zoomed = { x: world.position.x, y: world.position.y };
    fireEvent.pointerDown(app.canvas, { pointerId: 19, button: 1, clientX: 600, clientY: 250 });
    fireEvent.pointerMove(window, { pointerId: 19, clientX: 650, clientY: 280 });
    return { canvas: app.canvas,
      active: () => app.canvas.classList.contains("pixi-canvas--navigation-pan"),
      release: () => fireEvent.pointerUp(window, { pointerId: 19, clientX: 700, clientY: 300 }),
      changedContent: () => world.position.x !== zoomed.x || world.position.y !== zoomed.y };
  } },
  "a Photo drag between Frames": { mode: { kind: "normal" }, begin: async () => {
    const frameContentSwap = { disabled: false, dragThreshold: { x: 5, y: 5 },
      resolveTarget: vi.fn(async () => ({ kind: "frame" as const, frameId: "swap-frame-1" })),
      commit: vi.fn(async () => true), onError: vi.fn() };
    renderCanvas({ compositionPlan: structuredClone(frameContentSwapCorpus.before.composition), frameContentSwap });
    const app = await preparedCanvas();
    act(() => displayWithLabel("canvas-frame-swap-frame-0").emit("pointerdown", { button: 0, pointerId: 7,
      clientX: 100, clientY: 200, altKey: false, ctrlKey: false, metaKey: false, stopPropagation: vi.fn() }));
    fireEvent.pointerMove(window, { pointerId: 7, clientX: 300, clientY: 200 });
    return { canvas: app.canvas,
      active: () => app.canvas.classList.contains("pixi-canvas--frame-gesture"),
      release: () => fireEvent.pointerUp(window, { pointerId: 7, clientX: 300, clientY: 200 }),
      changedContent: () => frameContentSwap.commit.mock.calls.length > 0 };
  } },
};

// The production listener, mounted beside the real Canvas sessions.
function mountCanvasModeShortcuts(mode: AlbumCanvasMode) {
  const enter = vi.fn<(sheetId: string) => void>();
  const exit = vi.fn<() => void>();
  renderHook(() => useCanvasModeKeyboardShortcuts({
    implicitSheetId: "sheet-001", mode, onEnterSheetEditing: enter, onExitSheetEditing: exit,
  }));
  return { enter, exit };
}

const gestureNames = Object.keys(gestures);

test.each(gestureNames.flatMap((gesture) => ["Enter", "Escape"].map((key) => ({ gesture, key }))))(
  "$key during $gesture only cancels it; the next $key changes the Canvas mode as usual", async ({ gesture, key }) => {
    const { mode, begin } = gestures[gesture];
    const shortcuts = mountCanvasModeShortcuts(mode);
    const view = await begin();
    expect(view.active()).toBe(true);

    expect(fireEvent.keyDown(view.canvas, { key })).toBe(false);
    expect(view.active()).toBe(false);
    view.release();
    expect(view.changedContent()).toBe(false);
    expect(shortcuts.exit).not.toHaveBeenCalled();
    expect(shortcuts.enter).not.toHaveBeenCalled();

    fireEvent.keyDown(view.canvas, { key });
    if (mode.kind === "sheet-editing") expect(shortcuts.exit).toHaveBeenCalledOnce();
    else if (key === "Enter") expect(shortcuts.enter).toHaveBeenCalledExactlyOnceWith("sheet-001");
    else expect(shortcuts.enter).not.toHaveBeenCalled();
  },
);

test.each(gestureNames)("other keys during %s still cancel it and keep their own action", async (gesture) => {
  const { mode, begin } = gestures[gesture];
  const shortcuts = mountCanvasModeShortcuts(mode);
  const view = await begin();
  const laterShortcut = vi.fn();
  window.addEventListener("keydown", laterShortcut);

  // Ctrl+Enter adds a Sheet; only the plain Enter changes the Canvas mode.
  expect(fireEvent.keyDown(view.canvas, { key: "Enter", ctrlKey: true })).toBe(true);
  window.removeEventListener("keydown", laterShortcut);

  expect(view.active()).toBe(false);
  expect(laterShortcut).toHaveBeenCalledOnce();
  view.release();
  expect(view.changedContent()).toBe(false);
  expect(shortcuts.exit).not.toHaveBeenCalled();
  expect(shortcuts.enter).not.toHaveBeenCalled();
});
