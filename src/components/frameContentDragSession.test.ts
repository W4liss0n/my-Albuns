import type { FederatedPointerEvent } from "pixi.js";
import { afterEach, expect, test, vi } from "vitest";
import type { PhotoDropTarget } from "../domain/project";
import { frameContentSwapCorpus } from "../test/frameContentSwapPreview";
import type { AlbumCanvasProps } from "./albumCanvasContract";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import { FrameContentDragSession } from "./frameContentDragSession";

const sessions: FrameContentDragSession[] = [];
afterEach(() => { sessions.forEach((session) => session.destroy()); sessions.length = 0; vi.restoreAllMocks(); vi.useRealTimers(); });

function harness() {
  const canvas = document.createElement("canvas");
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue(new DOMRect(100, 100, 800, 400));
  canvas.setPointerCapture = vi.fn(); canvas.releasePointerCapture = vi.fn();
  const commit = vi.fn(async () => true);
  const resolveTarget = vi.fn(async (): Promise<PhotoDropTarget> => ({ kind: "frame", frameId: "swap-frame-4" }));
  const composition = structuredClone(frameContentSwapCorpus.before.composition);
  let input: AlbumCanvasProps = {
    projectId: "test", mode: { kind: "normal" }, composition,
    sheetBarMetadata: [], selectedFrameIds: [], focusedSheetId: null, centeredSheetId: null,
    continuousCanvasLayout: createContinuousCanvasLayout(composition.sheets), viewport: { offsetX: 0 },
    onSelectFrame: vi.fn(), onEditSheet: vi.fn(), onFocusSheet: vi.fn(), onCenteredSheetChange: vi.fn(),
    onViewportChange: vi.fn(), onTransformPreview: vi.fn(), onTransformCommit: async () => true,
    frameContentSwap: { disabled: false, dragThreshold: { x: 5, y: 5 }, commit, resolveTarget, onError: vi.fn() },
  };
  const resolvePoint = vi.fn((x: number) => ({ sheetId: "sheet-002", xUm: x, yUm: 150 }));
  const scroll = vi.fn();
  const session = new FrameContentDragSession(canvas, () => input, resolvePoint, scroll, vi.fn());
  sessions.push(session);
  const start = (overrides: Partial<FederatedPointerEvent> = {}, frame = "swap-frame-0") => session.start(frame, {
    button: 0, altKey: false, ctrlKey: false, metaKey: false, pointerId: 1, clientX: 200, clientY: 200,
    stopPropagation: vi.fn(), ...overrides,
  } as FederatedPointerEvent);
  const pointer = (type: string, x = 500, y = 200) => {
    const event = new Event(type);
    Object.assign(event, { pointerId: 1, clientX: x, clientY: y });
    window.dispatchEvent(event);
  };
  return { session, canvas, commit, resolveTarget, resolvePoint, scroll, start, pointer,
    get input() { return input; }, setInput(next: AlbumCanvasProps) { input = next; session.synchronize(input); } };
}

test("normal clicks below the Windows threshold do not swap or suppress selection", () => {
  const h = harness(); h.start(); h.pointer("pointermove", 205, 205); h.pointer("pointerup", 205, 205);
  expect(h.commit).not.toHaveBeenCalled(); expect(h.resolveTarget).not.toHaveBeenCalled();
  expect(h.session.ignoresTap).toBe(false);
  expect(h.session.preview).toBeNull();
});

test("ghost follows the pointer and a pending hover keeps feedback stable until the Core answers", async () => {
  const h = harness(); h.start(); h.pointer("pointermove");
  await Promise.resolve(); await Promise.resolve(); await Promise.resolve();
  let resolve!: (target: PhotoDropTarget) => void;
  h.resolveTarget.mockReturnValueOnce(new Promise((complete) => { resolve = complete; }));
  h.pointer("pointermove", 520, 220);
  expect(h.session.preview).toEqual({ sourceFrameId: "swap-frame-0", clientX: 520, clientY: 220 });
  expect(h.session.highlight).toEqual({ kind: "frame", frameId: "swap-frame-4" });
  expect(h.canvas.style.getPropertyValue("--frame-gesture-cursor")).toBe("grabbing");
  resolve({ kind: "invalid" }); await Promise.resolve();
  expect(h.session.highlight).toBeNull();
  expect(h.canvas.style.getPropertyValue("--frame-gesture-cursor")).toBe("grabbing");
  h.pointer("pointerup");
  expect(h.session.preview).toBeNull();
  expect(h.canvas.style.getPropertyValue("--frame-gesture-cursor")).toBe("");
});

test("a Photo drag highlights the Core target and queues release independently of pending hover", async () => {
  const h = harness(); h.start(); h.pointer("pointermove");
  await Promise.resolve();
  expect(h.session.highlight).toEqual({ kind: "frame", frameId: "swap-frame-4" });
  h.resolveTarget.mockReturnValueOnce(new Promise(() => {}));
  h.pointer("pointermove", 600); h.pointer("pointerup", 700);
  expect(h.commit).toHaveBeenCalledWith("swap-frame-0", { sheetId: "sheet-002", xUm: 700, yUm: 150 });
  expect(h.session.highlight).toBeNull(); expect(h.session.ignoresTap).toBe(true);
  expect(h.input.onSelectFrame).not.toHaveBeenCalled();
});

test.each(["escape", "pointercancel", "outside", "mode", "projection", "blocked", "blur"])("%s cancels without applying or accepting a stale hover", async (reason) => {
  const h = harness(); let resolve!: (target: PhotoDropTarget) => void;
  h.resolveTarget.mockReturnValueOnce(new Promise((yes) => { resolve = yes; }));
  h.start(); h.pointer("pointermove");
  if (reason === "escape") window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
  if (reason === "pointercancel") h.pointer("pointercancel");
  if (reason === "blur") window.dispatchEvent(new Event("blur"));
  if (reason === "mode") h.setInput({ ...h.input, mode: { kind: "sheet-editing", sheetId: "sheet-001" } });
  if (reason === "projection") h.setInput({ ...h.input, composition: structuredClone(h.input.composition) });
  if (reason === "blocked") h.setInput({ ...h.input, frameContentSwap: { ...h.input.frameContentSwap!, disabled: true } });
  h.pointer("pointerup", reason === "outside" ? 950 : 500);
  resolve({ kind: "frame", frameId: "swap-frame-4" }); await Promise.resolve();
  expect(h.commit).not.toHaveBeenCalled(); expect(h.session.highlight).toBeNull();
  expect(h.session.preview).toBeNull();
});

test.each(["alt", "control", "right", "empty", "editing", "blocked"])("%s does not start a content drag", (reason) => {
  const h = harness();
  if (reason === "editing") h.setInput({ ...h.input, mode: { kind: "sheet-editing", sheetId: "sheet-001" } });
  if (reason === "blocked") h.setInput({ ...h.input, frameContentSwap: { ...h.input.frameContentSwap!, disabled: true } });
  h.start({ altKey: reason === "alt", ctrlKey: reason === "control", button: reason === "right" ? 2 : 0 }, reason === "empty" ? "swap-frame-2" : "swap-frame-0");
  h.pointer("pointermove"); h.pointer("pointerup");
  expect(h.commit).not.toHaveBeenCalled(); expect(h.canvas.setPointerCapture).not.toHaveBeenCalled();
});

test("auto-scroll uses the current pointer near Canvas edges and stops on cancellation", () => {
  vi.useFakeTimers(); const h = harness(); h.start(); h.pointer("pointermove", 899);
  vi.advanceTimersByTime(60); expect(h.scroll).toHaveBeenCalled();
  h.pointer("pointercancel"); const calls = h.scroll.mock.calls.length;
  vi.advanceTimersByTime(100); expect(h.scroll).toHaveBeenCalledTimes(calls);
});
