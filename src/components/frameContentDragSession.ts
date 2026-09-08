import type { FederatedPointerEvent } from "pixi.js";
import type { PhotoDropTarget } from "../domain/project";
import type { AlbumCanvasProps, CanvasFrameContentSwap, CanvasPhotoDropPoint } from "./albumCanvasContract";
import { edgeAutoScrollVelocity } from "./edgeAutoScroll";

interface Drag {
  sourceFrameId: string;
  composition: AlbumCanvasProps["composition"];
  projectId: string;
  controls: CanvasFrameContentSwap;
  pointerId: number;
  origin: { clientX: number; clientY: number };
  point: { clientX: number; clientY: number };
  dragging: boolean;
  request: number;
  pending: boolean;
  desired: CanvasPhotoDropPoint | null;
}

/** Keeps a Photo in place until an authoritative drop, without changing normal selection. */
export class FrameContentDragSession {
  private drag: Drag | null = null;
  private target: PhotoDropTarget | null = null;
  private suppressedPointer: number | null = null;
  private tapTimer: ReturnType<typeof setTimeout> | undefined;
  private spaceHeld = false;
  private animation: number | undefined;
  private previousTime = 0;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly readInput: () => AlbumCanvasProps | null,
    private readonly resolvePoint: (x: number, y: number) => CanvasPhotoDropPoint | null,
    private readonly scroll: (deltaPx: number) => void,
    private readonly refresh: () => void,
  ) {
    window.addEventListener("pointermove", this.move);
    window.addEventListener("pointerup", this.finish);
    window.addEventListener("pointercancel", this.cancelPointer);
    canvas.addEventListener("lostpointercapture", this.cancelPointer);
    window.addEventListener("keydown", this.keyDown, true);
    window.addEventListener("keyup", this.keyUp, true);
    window.addEventListener("blur", this.blur);
    document.addEventListener("visibilitychange", this.visibilityChange);
  }

  get highlight() { return this.target; }
  get ignoresTap() { return this.suppressedPointer !== null; }

  start(sourceFrameId: string, event: FederatedPointerEvent) {
    const input = this.readInput();
    const controls = input?.frameContentSwap;
    if (event.button !== 0 || event.altKey || event.ctrlKey || event.metaKey || this.spaceHeld || this.drag ||
        input?.mode.kind !== "normal" || !controls?.dragThreshold || controls.disabled ||
        !input.composition.sheets.some((sheet) => sheet.frames.some((frame) => frame.frameId === sourceFrameId && frame.photo))) return;
    this.suppressedPointer = null;
    clearTimeout(this.tapTimer);
    this.drag = { sourceFrameId, controls, projectId: input.projectId, composition: input.composition,
      pointerId: event.pointerId, origin: { clientX: event.clientX, clientY: event.clientY },
      point: { clientX: event.clientX, clientY: event.clientY }, dragging: false,
      request: 0, pending: false, desired: null };
    event.stopPropagation();
  }

  synchronize(input: AlbumCanvasProps) {
    if (this.drag && (input.mode.kind !== "normal" || input.projectId !== this.drag.projectId ||
        input.composition !== this.drag.composition || input.frameContentSwap?.disabled ||
        !input.frameContentSwap?.dragThreshold)) this.reset();
  }

  reset() {
    const drag = this.drag;
    this.drag = null;
    this.target = null;
    if (this.animation !== undefined) cancelAnimationFrame(this.animation);
    this.animation = undefined;
    this.canvas.classList.remove("pixi-canvas--frame-gesture");
    this.canvas.style.removeProperty("--frame-gesture-cursor");
    if (drag) {
      try { this.canvas.releasePointerCapture(drag.pointerId); } catch { /* Capture already released. */ }
    }
  }

  destroy() {
    this.reset();
    clearTimeout(this.tapTimer);
    window.removeEventListener("pointermove", this.move);
    window.removeEventListener("pointerup", this.finish);
    window.removeEventListener("pointercancel", this.cancelPointer);
    this.canvas.removeEventListener("lostpointercapture", this.cancelPointer);
    window.removeEventListener("keydown", this.keyDown, true);
    window.removeEventListener("keyup", this.keyUp, true);
    window.removeEventListener("blur", this.blur);
    document.removeEventListener("visibilitychange", this.visibilityChange);
  }

  private pointInsideCanvas(point: { clientX: number; clientY: number }) {
    const bounds = this.canvas.getBoundingClientRect();
    return point.clientX >= bounds.left && point.clientX < bounds.right &&
      point.clientY >= bounds.top && point.clientY < bounds.bottom;
  }

  private readonly move = (event: PointerEvent) => {
    const drag = this.drag;
    if (!drag || event.pointerId !== drag.pointerId) return;
    drag.point = { clientX: event.clientX, clientY: event.clientY };
    if (!drag.dragging) {
      const threshold = drag.controls.dragThreshold!;
      if (Math.abs(event.clientX - drag.origin.clientX) <= threshold.x &&
          Math.abs(event.clientY - drag.origin.clientY) <= threshold.y) return;
      drag.dragging = true;
      this.suppressedPointer = drag.pointerId;
      // Ordinary clicks keep their native target and double-click sequence.
      this.canvas.setPointerCapture(drag.pointerId);
      this.canvas.classList.add("pixi-canvas--frame-gesture");
      this.canvas.style.setProperty("--frame-gesture-cursor", "no-drop");
      this.previousTime = performance.now();
      this.animation = requestAnimationFrame(this.tick);
    }
    this.updateTarget(drag);
  };

  private updateTarget(drag: Drag) {
    const point = this.pointInsideCanvas(drag.point) ? this.resolvePoint(drag.point.clientX, drag.point.clientY) : null;
    if (JSON.stringify(point) === JSON.stringify(drag.desired)) return;
    drag.desired = point;
    drag.request += 1;
    this.target = null;
    this.canvas.style.setProperty("--frame-gesture-cursor", "no-drop");
    this.refresh();
    this.requestTarget(drag);
  }

  private requestTarget(drag: Drag) {
    if (drag.pending || !drag.desired || this.drag !== drag) return;
    const request = drag.request;
    drag.pending = true;
    void drag.controls.resolveTarget(drag.desired).then((target) => {
      if (this.drag !== drag || drag.request !== request) return;
      this.target = target.kind === "frame" && target.frameId !== drag.sourceFrameId ? target : null;
      this.canvas.style.setProperty("--frame-gesture-cursor", this.target ? "move" : "no-drop");
      this.refresh();
    }).catch((error: unknown) => {
      if (this.drag !== drag || drag.request !== request) return;
      this.cancel();
      drag.controls.onError(error instanceof Error ? error.message : String(error));
    }).finally(() => {
      drag.pending = false;
      if (drag.request !== request) this.requestTarget(drag);
    });
  }

  private readonly tick = (time: number) => {
    const drag = this.drag;
    if (!drag?.dragging) return;
    const bounds = this.canvas.getBoundingClientRect();
    if (this.pointInsideCanvas(drag.point)) {
      const velocity = edgeAutoScrollVelocity({ axis: "horizontal", pointerPosition: drag.point.clientX,
        viewportStart: bounds.left, viewportEnd: bounds.right });
      if (velocity) this.scroll(velocity * Math.min(50, time - this.previousTime) / 1000);
      this.updateTarget(drag);
    }
    this.previousTime = time;
    if (this.drag === drag) this.animation = requestAnimationFrame(this.tick);
  };

  private readonly finish = (event: PointerEvent) => {
    const drag = this.drag;
    if (drag?.pointerId === event.pointerId) {
      const point = drag.dragging && this.pointInsideCanvas(event) ? this.resolvePoint(event.clientX, event.clientY) : null;
      this.reset();
      if (point) {
        // Queue synchronously at release; hover results are only visual feedback.
        void drag.controls.commit(drag.sourceFrameId, point).catch((error: unknown) =>
          drag.controls.onError(error instanceof Error ? error.message : String(error)));
      }
      this.refresh();
    }
    if (this.suppressedPointer === event.pointerId) this.deferTapReset();
  };

  private cancel() { this.reset(); this.refresh(); }
  private deferTapReset() {
    clearTimeout(this.tapTimer);
    this.tapTimer = setTimeout(() => { this.suppressedPointer = null; }, 0);
  }
  private readonly cancelPointer = (event: PointerEvent) => {
    if (this.drag?.pointerId === event.pointerId) this.cancel();
    // Keep a cancelled drag suppressed until release, including Escape while held.
    if (event.type === "pointercancel") this.deferTapReset();
  };
  private readonly keyDown = (event: KeyboardEvent) => {
    if (event.code === "Space") this.spaceHeld = true;
    if (!this.drag) return;
    if (event.key === "Escape") { event.preventDefault(); event.stopImmediatePropagation(); }
    this.cancel();
  };
  private readonly keyUp = (event: KeyboardEvent) => { if (event.code === "Space") this.spaceHeld = false; };
  private readonly blur = () => { this.spaceHeld = false; this.cancel(); this.deferTapReset(); };
  private readonly visibilityChange = () => { if (document.hidden) this.blur(); };
}
