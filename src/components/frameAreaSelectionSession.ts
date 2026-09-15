import { Graphics, type Application, type FederatedPointerEvent } from "pixi.js";
import type { AlbumCanvasProps } from "./albumCanvasContract";
import { pixiColor } from "./pixiColor";
import { SHEET_VISUAL_STYLE } from "./sheetVisualStyle";

interface Point { x: number; y: number }
interface Gesture {
  pointerId: number;
  origin: Point;
  point: Point;
  sheetOrigin: Point;
  input: AlbumCanvasProps;
  bounds: DOMRect;
  screen: Point;
  ctrlKey: boolean;
  dragging: boolean;
  selected: readonly string[];
}

/** Transient selection only; the confirmed selection changes once, on release. */
export class FrameAreaSelectionSession {
  private readonly box = new Graphics();
  private gesture: Gesture | null = null;
  private suppressTap = false;
  private pointerAwaitingRelease: number | null = null;
  private tapTimer: ReturnType<typeof setTimeout> | undefined;
  private spaceHeld = false;

  constructor(
    private readonly app: Application,
    private readonly readInput: () => AlbumCanvasProps | null,
    private readonly resolveSheetPoint: (x: number, y: number) => Point | null,
    private readonly refresh: () => void,
  ) {
    this.box.label = "frame-area-selection";
    this.box.eventMode = "none";
    this.box.visible = false;
    app.stage.addChild(this.box);
    window.addEventListener("pointerdown", this.clearCancelledPress, true);
    window.addEventListener("pointermove", this.move);
    // Settle before Pixi emits the release's pointertap.
    window.addEventListener("pointerup", this.finish, true);
    window.addEventListener("pointercancel", this.cancelPointer);
    app.canvas.addEventListener("lostpointercapture", this.cancelPointer);
    window.addEventListener("keydown", this.keyDown, true);
    window.addEventListener("keyup", this.keyUp, true);
    window.addEventListener("blur", this.blur);
    document.addEventListener("visibilitychange", this.visibilityChange);
  }

  start(event: FederatedPointerEvent) {
    const input = this.readInput();
    if (event.button !== 0 || this.spaceHeld || this.gesture || input?.mode.kind !== "sheet-editing" ||
        !input.onSelectFrames || input.frameGeometry?.disabled || !input.frameGeometry?.dragThreshold || input.mediaDrag) return;
    const sheetOrigin = this.resolveSheetPoint(event.clientX, event.clientY);
    const bounds = this.app.canvas.getBoundingClientRect();
    if (!sheetOrigin || bounds.width <= 0 || bounds.height <= 0) return;
    const point = { x: event.clientX, y: event.clientY };
    this.gesture = { input, pointerId: event.pointerId, origin: point, point, sheetOrigin, bounds,
      screen: { x: this.app.screen.width, y: this.app.screen.height }, ctrlKey: event.ctrlKey,
      dragging: false, selected: input.selectedFrameIds };
    clearTimeout(this.tapTimer);
    this.suppressTap = true;
    this.pointerAwaitingRelease = event.pointerId;
    event.stopPropagation();
    this.app.canvas.setPointerCapture(event.pointerId);
  }

  get ignoresTap() { return this.suppressTap; }
  get previewSelection() { return this.gesture?.dragging ? this.gesture.selected : null; }

  synchronize(input: AlbumCanvasProps) {
    const gesture = this.gesture;
    if (!gesture) return;
    const bounds = this.app.canvas.getBoundingClientRect();
    if (input.mode.kind !== "sheet-editing" || gesture.input.mode.kind !== "sheet-editing" ||
        input.mode.sheetId !== gesture.input.mode.sheetId || input.projectId !== gesture.input.projectId ||
        input.composition !== gesture.input.composition || input.selectedFrameIds !== gesture.input.selectedFrameIds ||
        input.frameGeometry?.disabled || !input.frameGeometry?.dragThreshold || input.mediaDrag ||
        bounds.x !== gesture.bounds.x || bounds.y !== gesture.bounds.y || bounds.width !== gesture.bounds.width ||
        bounds.height !== gesture.bounds.height || this.app.screen.width !== gesture.screen.x ||
        this.app.screen.height !== gesture.screen.y) this.reset();
  }

  reset() {
    const gesture = this.gesture;
    this.gesture = null;
    this.box.clear();
    this.box.visible = false;
    if (gesture) {
      try { this.app.canvas.releasePointerCapture(gesture.pointerId); } catch { /* Already released. */ }
    }
  }

  destroy() {
    this.reset();
    clearTimeout(this.tapTimer);
    window.removeEventListener("pointerdown", this.clearCancelledPress, true);
    window.removeEventListener("pointermove", this.move);
    window.removeEventListener("pointerup", this.finish, true);
    window.removeEventListener("pointercancel", this.cancelPointer);
    this.app.canvas.removeEventListener("lostpointercapture", this.cancelPointer);
    window.removeEventListener("keydown", this.keyDown, true);
    window.removeEventListener("keyup", this.keyUp, true);
    window.removeEventListener("blur", this.blur);
    document.removeEventListener("visibilitychange", this.visibilityChange);
    this.box.destroy();
  }

  private update(point: Point, ctrlKey: boolean) {
    const gesture = this.gesture;
    if (!gesture) return;
    gesture.point = point;
    gesture.ctrlKey = ctrlKey;
    const threshold = gesture.input.frameGeometry!.dragThreshold!;
    if (!gesture.dragging && Math.abs(point.x - gesture.origin.x) <= threshold.x &&
        Math.abs(point.y - gesture.origin.y) <= threshold.y) return;
    const end = this.resolveSheetPoint(point.x, point.y);
    if (!end || gesture.input.mode.kind !== "sheet-editing") return;
    gesture.dragging = true;
    const left = Math.min(gesture.sheetOrigin.x, end.x);
    const right = Math.max(gesture.sheetOrigin.x, end.x);
    const top = Math.min(gesture.sheetOrigin.y, end.y);
    const bottom = Math.max(gesture.sheetOrigin.y, end.y);
    const sheetId = gesture.input.mode.sheetId;
    const sheet = gesture.input.composition.sheets.find((item) => item.sheetId === sheetId);
    gesture.selected = sheet?.frames.filter((frame) => {
      const rect = frame.clipRect;
      return (ctrlKey && gesture.input.selectedFrameIds.includes(frame.frameId)) ||
        (rect.x <= right && rect.x + rect.width >= left && rect.y <= bottom && rect.y + rect.height >= top);
    }).map((frame) => frame.frameId) ?? [];
    const { bounds, screen, origin } = gesture;
    const color = pixiColor(SHEET_VISUAL_STYLE.frameSelection.handleOutline);
    this.box.clear().rect(
      (Math.min(origin.x, point.x) - bounds.left) * screen.x / bounds.width,
      (Math.min(origin.y, point.y) - bounds.top) * screen.y / bounds.height,
      Math.abs(point.x - origin.x) * screen.x / bounds.width,
      Math.abs(point.y - origin.y) * screen.y / bounds.height,
    ).fill({ color, alpha: 0.1 }).stroke({ color, width: 1, pixelLine: true });
    this.box.visible = true;
    this.refresh();
  }

  private readonly move = (event: PointerEvent) => {
    if (event.pointerId === this.gesture?.pointerId) this.update({ x: event.clientX, y: event.clientY }, event.ctrlKey);
  };
  private readonly finish = (event: PointerEvent) => {
    if (event.pointerId === this.pointerAwaitingRelease) {
      this.pointerAwaitingRelease = null;
      clearTimeout(this.tapTimer);
      this.tapTimer = setTimeout(() => { this.suppressTap = false; }, 0);
    }
    if (event.pointerId !== this.gesture?.pointerId) return;
    this.update({ x: event.clientX, y: event.clientY }, event.ctrlKey);
    const gesture = this.gesture;
    if (!gesture) return;
    const selected = gesture.dragging ? gesture.selected : gesture.ctrlKey ? gesture.input.selectedFrameIds : [];
    this.reset();
    this.readInput()?.onSelectFrames?.(selected);
    this.refresh();
  };
  private readonly clearCancelledPress = () => {
    if (!this.gesture) {
      this.suppressTap = false;
      this.pointerAwaitingRelease = null;
      clearTimeout(this.tapTimer);
    }
  };
  private readonly cancel = () => { this.reset(); this.refresh(); };
  private readonly cancelPointer = (event: PointerEvent) => {
    if (event.pointerId === this.gesture?.pointerId) this.cancel();
  };
  private readonly keyDown = (event: KeyboardEvent) => {
    if (event.code === "Space") this.spaceHeld = true;
    if (!this.gesture) return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopImmediatePropagation();
      this.cancel();
    } else if (event.key === "Control") this.update(this.gesture.point, event.ctrlKey);
    else this.cancel();
  };
  private readonly keyUp = (event: KeyboardEvent) => {
    if (event.code === "Space") this.spaceHeld = false;
    if (this.gesture && event.key === "Control") this.update(this.gesture.point, event.ctrlKey);
  };
  private readonly blur = () => { this.spaceHeld = false; this.cancel(); };
  private readonly visibilityChange = () => { if (document.hidden) this.blur(); };
}
