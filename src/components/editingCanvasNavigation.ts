import { matchProjectCommandShortcut } from "../application/projectCommandCatalog";
import type { AlbumCanvasProps } from "./albumCanvasContract";
import { isTextEntryTarget } from "./isTextEntryTarget";
import { boundedEditingTransform, fittedEditingTransform, resizedEditingTransform,
  zoomEditingTransform, type EditingCanvasFit, type EditingCanvasTransform, type ViewPoint } from "./editingCanvasViewport";

interface PanGesture {
  pointerId: number;
  origin: ViewPoint;
  transform: EditingCanvasTransform;
}

/** Owns only the isolated Canvas camera; no Project commands or history entries. */
export class EditingCanvasNavigation {
  private scope: string | null = null;
  private fit: EditingCanvasFit | null = null;
  private transform: EditingCanvasTransform = { zoom: 1, x: 0, y: 0 };
  private pan: PanGesture | null = null;
  private spaceHeld = false;
  private fitRequest: number | undefined;
  private pointerAwaitingRelease: number | null = null;

  constructor(private readonly canvas: HTMLCanvasElement,
    private readonly readInput: () => AlbumCanvasProps | null,
    private readonly beforeTransform: () => void,
    private readonly refresh: () => void,
  ) {
    // Capture before Pixi dispatches a press into Frames or the area-selection tool.
    window.addEventListener("pointerdown", this.pointerDown, true);
    window.addEventListener("pointermove", this.pointerMove, true);
    window.addEventListener("pointerup", this.pointerUp, true);
    window.addEventListener("pointercancel", this.pointerCancel, true);
    canvas.addEventListener("lostpointercapture", this.pointerCancel);
    window.addEventListener("keydown", this.keyDown, true);
    window.addEventListener("keyup", this.keyUp, true);
    window.addEventListener("wheel", this.wheel, { capture: true, passive: false });
    window.addEventListener("blur", this.blur);
    document.addEventListener("visibilitychange", this.visibilityChange);
    canvas.addEventListener("auxclick", this.auxClick);
  }

  synchronize(input: AlbumCanvasProps, fit: EditingCanvasFit) {
    const scope = input.mode.kind === "sheet-editing" ? JSON.stringify([input.projectId, input.mode.sheetId]) : null;
    const resized = this.fit && Object.keys(fit).some((key) =>
      fit[key as keyof EditingCanvasFit] !== this.fit![key as keyof EditingCanvasFit]);
    if (scope !== this.scope || resized || !this.enabled) this.cancelPan();
    if (scope !== this.scope) {
      this.transform = fittedEditingTransform(fit);
      this.spaceHeld = false;
    } else if (this.fit && resized) this.transform = resizedEditingTransform(this.fit, fit, this.transform);
    this.scope = scope;
    this.fit = fit;
    if (input.editingNavigation?.fitRequest !== this.fitRequest) {
      this.fitRequest = input.editingNavigation?.fitRequest;
      if (this.enabled) {
        this.cancelPan();
        this.beforeTransform();
        this.transform = fittedEditingTransform(fit);
      }
    }
    this.updateCursor();
    this.canvas.dataset.editingZoom = scope ? String(this.transform.zoom) : "";
    return this.transform;
  }

  private get enabled() {
    const input = this.readInput();
    return input?.mode.kind === "sheet-editing" && !input.editingNavigation?.disabled &&
      !input.frameGeometry?.disabled && !input.mediaDrag;
  }

  private point(clientX: number, clientY: number): ViewPoint | null {
    const bounds = this.canvas.getBoundingClientRect();
    if (!this.fit || bounds.width <= 0 || bounds.height <= 0) return null;
    return { x: (clientX - bounds.left) * this.fit.width / bounds.width,
      y: (clientY - bounds.top) * this.fit.height / bounds.height };
  }

  private changeZoom(factor: number, anchor: ViewPoint) {
    if (!this.fit) return;
    this.cancelPan();
    this.beforeTransform();
    this.transform = zoomEditingTransform(this.fit, this.transform, factor, anchor);
    this.refresh();
  }

  private readonly wheel = (event: WheelEvent) => {
    if (event.target !== this.canvas || this.readInput()?.mode.kind !== "sheet-editing" || !event.ctrlKey) return;
    // Never let a Canvas zoom shortcut resize the WebView instead.
    event.preventDefault();
    event.stopImmediatePropagation();
    if (!this.enabled || event.altKey || event.metaKey || event.deltaY === 0) return;
    const anchor = this.point(event.clientX, event.clientY);
    if (!anchor) return;
    const pixels = event.deltaY * (event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? this.fit!.height : 1);
    this.changeZoom(Math.exp(-Math.max(-600, Math.min(600, pixels)) * 0.002), anchor);
  };

  private readonly keyDown = (event: KeyboardEvent) => {
    if (this.pan && event.key === "Escape") {
      event.preventDefault();
      event.stopImmediatePropagation();
      this.cancelPan();
      this.refresh();
      return;
    }
    if (event.defaultPrevented || !this.keyboardTarget(event.target) || this.readInput()?.mode.kind !== "sheet-editing") return;
    if (event.code === "Space" && !event.ctrlKey && !event.altKey && !event.metaKey) {
      event.preventDefault();
      if (this.enabled) this.spaceHeld = true;
      this.updateCursor();
      return;
    }
    const command = matchProjectCommandShortcut(event, "sheet");
    if (command !== "fit-sheet" && command !== "canvas-zoom-in" && command !== "canvas-zoom-out") {
      if (this.pan) { this.cancelPan(); this.refresh(); }
      return;
    }
    event.preventDefault();
    event.stopImmediatePropagation();
    if (!this.enabled || !this.fit || event.repeat) return;
    if (command === "fit-sheet") {
      this.cancelPan();
      this.beforeTransform();
      this.transform = fittedEditingTransform(this.fit);
      this.refresh();
    } else this.changeZoom(command === "canvas-zoom-in" ? 1.2 : 1 / 1.2,
      { x: this.fit.width / 2, y: this.fit.height / 2 });
  };

  private keyboardTarget(target: EventTarget | null) {
    if (isTextEntryTarget(target)) return false;
    if (!(target instanceof Element)) return true;
    return target === document.body || target === this.canvas || target.closest(".canvas-host") !== null;
  }

  private readonly keyUp = (event: KeyboardEvent) => {
    if (event.code === "Space") { this.spaceHeld = false; this.updateCursor(); }
  };

  private readonly pointerDown = (event: PointerEvent) => {
    if (!this.pan) this.pointerAwaitingRelease = null;
    if (event.target !== this.canvas || !this.enabled ||
        !(event.button === 1 || (event.button === 0 && this.spaceHeld))) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    this.canvas.focus({ preventScroll: true });
    if (this.pan) return;
    this.pointerAwaitingRelease = event.pointerId;
    if (this.transform.zoom <= 1) return;
    const origin = this.point(event.clientX, event.clientY);
    if (!origin) return;
    this.beforeTransform();
    this.pan = { origin, pointerId: event.pointerId, transform: { ...this.transform } };
    this.canvas.setPointerCapture(event.pointerId);
    this.updateCursor();
    this.refresh();
  };

  private movePan(event: PointerEvent) {
    const point = this.point(event.clientX, event.clientY);
    if (!this.pan || !point || !this.fit) return;
    this.transform = boundedEditingTransform(this.fit, { zoom: this.pan.transform.zoom,
      x: this.pan.transform.x + point.x - this.pan.origin.x,
      y: this.pan.transform.y + point.y - this.pan.origin.y });
    this.refresh();
  }

  private readonly pointerMove = (event: PointerEvent) => {
    if (event.pointerId !== this.pan?.pointerId) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    this.movePan(event);
  };

  private readonly pointerUp = (event: PointerEvent) => {
    if (event.pointerId !== this.pointerAwaitingRelease) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    this.pointerAwaitingRelease = null;
    if (event.pointerId === this.pan?.pointerId) { this.movePan(event); this.releasePan(); }
  };

  private releasePan() {
    const pan = this.pan;
    this.pan = null;
    if (pan) {
      try { this.canvas.releasePointerCapture(pan.pointerId); } catch { /* Capture already ended. */ }
    }
    this.updateCursor();
  }

  private cancelPan() {
    if (this.pan) this.transform = this.pan.transform;
    this.releasePan();
  }

  private readonly pointerCancel = (event: PointerEvent) => {
    if (event.pointerId === this.pan?.pointerId) { this.cancelPan(); this.refresh(); }
  };
  private readonly auxClick = (event: MouseEvent) => {
    if (event.button === 1 && this.readInput()?.mode.kind === "sheet-editing") event.preventDefault();
  };
  private readonly blur = () => { this.suspend(); this.refresh(); };
  private readonly visibilityChange = () => { if (document.hidden) this.blur(); };

  private updateCursor() {
    this.canvas.classList.toggle("pixi-canvas--navigation-ready", this.enabled && this.spaceHeld && this.transform.zoom > 1);
    this.canvas.classList.toggle("pixi-canvas--navigation-pan", this.pan !== null);
  }

  suspend() { this.spaceHeld = false; this.cancelPan(); }

  destroy() {
    this.suspend();
    window.removeEventListener("pointerdown", this.pointerDown, true);
    window.removeEventListener("pointermove", this.pointerMove, true);
    window.removeEventListener("pointerup", this.pointerUp, true);
    window.removeEventListener("pointercancel", this.pointerCancel, true);
    this.canvas.removeEventListener("lostpointercapture", this.pointerCancel);
    window.removeEventListener("keydown", this.keyDown, true);
    window.removeEventListener("keyup", this.keyUp, true);
    window.removeEventListener("wheel", this.wheel, true);
    window.removeEventListener("blur", this.blur);
    document.removeEventListener("visibilitychange", this.visibilityChange);
    this.canvas.removeEventListener("auxclick", this.auxClick);
    delete this.canvas.dataset.editingZoom;
  }
}
