import type { FederatedPointerEvent } from "pixi.js";

import type {
  ComposedFrame,
  ComposedSheet,
  FrameGeometryEdit,
  FrameResizeHandle,
} from "../domain/project";
import type { AlbumCanvasProps, CanvasFrameGeometry } from "./albumCanvasContract";
import { MICROMETER_TO_CANVAS_PIXEL } from "./canvasGeometry";

interface PointerPosition {
  clientX: number;
  clientY: number;
  shiftKey: boolean;
  altKey: boolean;
}

interface FrameGesture {
  frame: ComposedFrame;
  sheetId: string;
  sourceSignature: string;
  controls: CanvasFrameGeometry;
  pointerId: number;
  origin: PointerPosition;
  point: PointerPosition;
  handle: FrameResizeHandle | null;
  umPerPixelX: number;
  umPerPixelY: number;
  scale: number;
  phase: "pressed" | "dragging" | "committing";
  desired: FrameGeometryEdit | null;
  inFlight: boolean;
  preview: ComposedFrame | null;
}

/** Owns one pointer gesture. Only the Core may calculate its proposed geometry. */
export class FrameInteractionSession {
  private gesture: FrameGesture | null = null;
  private spaceHeld = false;
  private suppressTap = false;
  private tapTimer: ReturnType<typeof setTimeout> | undefined;

  constructor(
    private readonly canvas: HTMLCanvasElement,
    private readonly readContext: () => {
      input: AlbumCanvasProps | null;
      canvasScale: number;
      screen: { width: number; height: number };
    },
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

  start(
    frameId: string,
    handle: FrameResizeHandle | null,
    event: FederatedPointerEvent,
  ) {
    const { input, canvasScale, screen } = this.readContext();
    const controls = input?.frameGeometry;
    if (
      event.button !== 0 || this.spaceHeld || this.gesture ||
      input?.mode.kind !== "sheet-editing" ||
      !controls?.dragThreshold || controls.disabled
    ) return;
    // Resolve against the confirmed composition, never a previously painted preview.
    const editingSheetId = input.mode.sheetId;
    const confirmedSheet = input.composition.sheets.find(
      (item) => item.sheetId === editingSheetId,
    );
    const frame = confirmedSheet?.frames.find((item) => item.frameId === frameId);
    const layoutLocked = input.sheetBarMetadata.find(
      (item) => item.sheetId === editingSheetId,
    )?.layoutLocked;
    if (!confirmedSheet || !frame || layoutLocked) return;
    const bounds = this.canvas.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0 || canvasScale <= 0) return;
    this.suppressTap = false;
    clearTimeout(this.tapTimer);
    this.gesture = {
      frame,
      sheetId: editingSheetId,
      sourceSignature: sourceSignature(input, confirmedSheet, frame),
      controls,
      pointerId: event.pointerId,
      origin: pointerPosition(event),
      point: pointerPosition(event),
      handle,
      umPerPixelX: screen.width / bounds.width / canvasScale / MICROMETER_TO_CANVAS_PIXEL,
      umPerPixelY: screen.height / bounds.height / canvasScale / MICROMETER_TO_CANVAS_PIXEL,
      scale: canvasScale,
      phase: "pressed",
      desired: null,
      inFlight: false,
      preview: null,
    };
    event.stopPropagation();
    this.canvas.setPointerCapture(event.pointerId);
  }

  synchronize(input: AlbumCanvasProps, scale: number) {
    const gesture = this.gesture;
    if (!gesture) return;
    const sheet = input.composition.sheets.find(
      (item) => item.sheetId === gesture.sheetId,
    );
    const frame = sheet?.frames.find(
      (item) => item.frameId === gesture.frame.frameId,
    );
    if (
      input.mode.kind !== "sheet-editing" || input.mode.sheetId !== gesture.sheetId ||
      input.frameGeometry?.disabled || !input.frameGeometry?.dragThreshold ||
      !sheet || !frame || scale !== gesture.scale ||
      sourceSignature(input, sheet, frame) !== gesture.sourceSignature
    ) this.reset();
  }

  present(sheets: readonly ComposedSheet[]): readonly ComposedSheet[] {
    const gesture = this.gesture;
    if (!gesture?.preview) return sheets;
    const preview = gesture.preview;
    return sheets.map((sheet) => sheet.sheetId !== gesture.sheetId ? sheet : {
      ...sheet,
      frames: sheet.frames.map((frame) =>
        frame.frameId === preview.frameId ? preview : frame),
    });
  }

  get ignoresTap() {
    return this.suppressTap;
  }

  reset() {
    const gesture = this.gesture;
    this.gesture = null;
    if (gesture) this.release(gesture);
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

  private updatePoint(point: PointerPosition) {
    const gesture = this.gesture;
    if (!gesture || gesture.phase === "committing") return;
    gesture.point = pointerPosition(point);
    if (gesture.phase === "pressed") {
      const threshold = gesture.controls.dragThreshold!;
      if (
        Math.abs(point.clientX - gesture.origin.clientX) <= threshold.x &&
        Math.abs(point.clientY - gesture.origin.clientY) <= threshold.y
      ) return;
      gesture.phase = "dragging";
      this.suppressTap = true;
      this.readContext().input?.onSelectFrame(gesture.frame.frameId);
    }
    gesture.desired = this.edit(gesture);
    this.requestPreview(gesture);
  }

  private edit(gesture: FrameGesture): FrameGeometryEdit {
    const deltaXUm = Math.round(
      (gesture.point.clientX - gesture.origin.clientX) * gesture.umPerPixelX,
    );
    const deltaYUm = Math.round(
      (gesture.point.clientY - gesture.origin.clientY) * gesture.umPerPixelY,
    );
    return {
      frameId: gesture.frame.frameId,
      expectedRect: gesture.frame.clipRect,
      gesture: gesture.handle === null
        ? { kind: "move", deltaXUm, deltaYUm }
        : {
            kind: "resize", handle: gesture.handle, deltaXUm, deltaYUm,
            preserveAspectRatio: gesture.point.shiftKey,
            fromCenter: gesture.point.altKey,
          },
    };
  }

  private requestPreview(gesture: FrameGesture) {
    if (gesture.inFlight || !gesture.desired || gesture.phase !== "dragging") return;
    const edit = gesture.desired;
    gesture.inFlight = true;
    void gesture.controls.preview(edit).then((preview) => {
      if (this.gesture !== gesture || gesture.phase !== "dragging") return;
      gesture.preview = preview;
      this.refresh();
    }).catch((error: unknown) => {
      if (this.gesture !== gesture || gesture.phase !== "dragging") return;
      this.cancel();
      gesture.controls.onError(errorMessage(error));
    }).finally(() => {
      gesture.inFlight = false;
      if (this.gesture === gesture && gesture.desired !== edit) this.requestPreview(gesture);
    });
  }

  private readonly move = (event: PointerEvent) => {
    if (this.gesture?.pointerId === event.pointerId) this.updatePoint(event);
  };

  private readonly finish = (event: PointerEvent) => {
    const gesture = this.gesture;
    if (
      !gesture || gesture.pointerId !== event.pointerId || gesture.phase === "committing"
    ) return;
    if (gesture.phase === "pressed") {
      this.reset();
      return;
    }
    gesture.point = pointerPosition(event);
    gesture.phase = "committing";
    this.release(gesture);
    this.deferTapReset();
    // Queue the final edit immediately, before a following Save/Undo can enter the shared queue.
    void gesture.controls.commit(this.edit(gesture)).catch((error: unknown) => {
      if (this.gesture === gesture) gesture.controls.onError(errorMessage(error));
    }).finally(() => {
      if (this.gesture !== gesture) return;
      this.reset();
      this.refresh();
    });
  };

  private release(gesture: FrameGesture) {
    // The browser may already have released capture after cancellation or context loss.
    try {
      this.canvas.releasePointerCapture(gesture.pointerId);
    } catch {
      // Already released.
    }
  }

  private cancel() {
    if (!this.gesture || this.gesture.phase === "committing") return;
    this.suppressTap = true;
    this.reset();
    this.deferTapReset();
    this.refresh();
  }

  private deferTapReset() {
    clearTimeout(this.tapTimer);
    this.tapTimer = setTimeout(() => { this.suppressTap = false; }, 0);
  }

  private readonly cancelPointer = (event: PointerEvent) => {
    if (event.pointerId === this.gesture?.pointerId) this.cancel();
  };

  private readonly keyDown = (event: KeyboardEvent) => {
    if (event.code === "Space") this.spaceHeld = true;
    const gesture = this.gesture;
    if (!gesture || gesture.phase === "committing") return;
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopImmediatePropagation();
      this.cancel();
    } else if (event.key === "Shift" || event.key === "Alt") {
      event.preventDefault();
      this.updatePoint({
        ...gesture.point, shiftKey: event.shiftKey, altKey: event.altKey,
      });
    } else if (event.key !== "Control" && event.key !== "Meta") this.cancel();
  };

  private readonly keyUp = (event: KeyboardEvent) => {
    if (event.code === "Space") this.spaceHeld = false;
    if (this.gesture && (event.key === "Shift" || event.key === "Alt")) {
      this.updatePoint({
        ...this.gesture.point, shiftKey: event.shiftKey, altKey: event.altKey,
      });
    }
  };

  private readonly blur = () => {
    this.spaceHeld = false;
    this.cancel();
  };

  private readonly visibilityChange = () => {
    if (document.hidden) this.blur();
  };
}

function pointerPosition(point: PointerPosition): PointerPosition {
  return {
    clientX: point.clientX, clientY: point.clientY,
    shiftKey: point.shiftKey, altKey: point.altKey,
  };
}

function sourceSignature(
  input: AlbumCanvasProps,
  sheet: ComposedSheet,
  frame: ComposedFrame,
) {
  return JSON.stringify([
    input.projectId, sheet.widthUm, sheet.heightUm, frame,
    input.composition.frameBorder,
    input.sheetBarMetadata.find((item) => item.sheetId === sheet.sheetId)?.layoutLocked,
  ]);
}

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}
