import {
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
} from "react";

export const SHEET_REORDER_POINTER_THRESHOLD_PX = 5;

export interface SheetReorderPointerPosition {
  readonly clientX: number;
  readonly clientY: number;
}

export interface SheetReorderPointerState
  extends SheetReorderPointerPosition {
  readonly active: boolean;
  readonly sourceId: string;
  readonly startX: number;
  readonly startY: number;
}

interface ActivePointer extends SheetReorderPointerState {
  readonly captureTarget: HTMLElement;
  readonly pointerId: number;
  readonly previewed: boolean;
  readonly resolvedTargetIndex: number | null;
}

export interface SheetPointerReorderOptions {
  readonly enabled: boolean;
  readonly onActivate: (sourceId: string) => void;
  readonly onCancel: () => void;
  readonly onDrop: () => void;
  readonly onFinish?: () => void;
  readonly onMove?: (position: SheetReorderPointerPosition) => void;
  readonly onPreview: (sourceId: string, targetIndex: number) => void;
  readonly resolveTarget: (
    position: SheetReorderPointerPosition,
  ) => number | null;
  readonly targetGeometryRevision?: unknown;
  readonly validRelease: (
    position: SheetReorderPointerPosition,
  ) => boolean;
}

export interface SheetPointerReorder {
  readonly pointer: SheetReorderPointerState | null;
  readonly begin: (
    event: ReactPointerEvent<HTMLElement>,
    sourceId: string,
    captureTarget?: HTMLElement | null,
  ) => void;
  readonly move: (event: ReactPointerEvent<HTMLElement>) => void;
  readonly end: (event: ReactPointerEvent<HTMLElement>) => void;
  readonly cancel: (event: ReactPointerEvent<HTMLElement>) => void;
  readonly lostCapture: (event: ReactPointerEvent<HTMLElement>) => void;
  readonly consumeClickSuppression: (sourceId: string) => boolean;
  readonly refreshTarget: () => void;
}

export function useSheetPointerReorder(
  options: SheetPointerReorderOptions,
): SheetPointerReorder {
  const optionsRef = useRef(options);
  optionsRef.current = options;
  const activeRef = useRef<ActivePointer | null>(null);
  const suppressedClickSourceRef = useRef<string | null>(null);
  const clickSuppressionTimerRef = useRef<number | null>(null);
  const [pointer, setPointer] = useState<SheetReorderPointerState | null>(null);

  function clearClickSuppression() {
    if (clickSuppressionTimerRef.current !== null) {
      window.clearTimeout(clickSuppressionTimerRef.current);
    }
    clickSuppressionTimerRef.current = null;
    suppressedClickSourceRef.current = null;
  }

  function scheduleClickSuppression(sourceId: string) {
    clearClickSuppression();
    suppressedClickSourceRef.current = sourceId;
    clickSuppressionTimerRef.current = window.setTimeout(() => {
      if (suppressedClickSourceRef.current === sourceId) {
        suppressedClickSourceRef.current = null;
      }
      clickSuppressionTimerRef.current = null;
    }, 0);
  }

  function releasePointerCapture(active: ActivePointer) {
    try {
      active.captureTarget.releasePointerCapture?.(active.pointerId);
    } catch {
      // Capture may already be released by the browser during cancellation.
    }
  }

  function finishPointer(
    pointerId: number,
    outcome: "cancel" | "drop" | "none",
  ) {
    const active = activeRef.current;
    if (!active || active.pointerId !== pointerId) return;
    activeRef.current = null;
    setPointer(null);
    releasePointerCapture(active);
    optionsRef.current.onFinish?.();
    if (outcome === "drop") optionsRef.current.onDrop();
    if (outcome === "cancel") optionsRef.current.onCancel();
  }

  function cancelActivePointer() {
    const active = activeRef.current;
    if (!active) return;
    if (active.active) {
      scheduleClickSuppression(active.sourceId);
    }
    finishPointer(
      active.pointerId,
      active.active ? "cancel" : "none",
    );
  }

  useEffect(() => {
    const cancelOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || !activeRef.current) return;
      event.preventDefault();
      event.stopImmediatePropagation();
      cancelActivePointer();
    };
    window.addEventListener("keydown", cancelOnEscape, true);
    return () => {
      window.removeEventListener("keydown", cancelOnEscape, true);
      clearClickSuppression();
      const active = activeRef.current;
      activeRef.current = null;
      if (!active) return;
      releasePointerCapture(active);
      optionsRef.current.onFinish?.();
      if (active.active) optionsRef.current.onCancel();
    };
  }, []);

  useEffect(() => {
    if (!options.enabled) cancelActivePointer();
  }, [options.enabled]);

  useEffect(() => {
    refreshTarget();
  }, [options.targetGeometryRevision]);

  function begin(
    event: ReactPointerEvent<HTMLElement>,
    sourceId: string,
    captureTarget?: HTMLElement | null,
  ) {
    if (
      !optionsRef.current.enabled ||
      event.button !== 0
    ) {
      return;
    }
    cancelActivePointer();
    clearClickSuppression();
    const stableCaptureTarget = captureTarget ?? event.currentTarget;
    const next: ActivePointer = {
      active: false,
      captureTarget: stableCaptureTarget,
      clientX: event.clientX,
      clientY: event.clientY,
      pointerId: event.pointerId,
      previewed: false,
      resolvedTargetIndex: null,
      sourceId,
      startX: event.clientX,
      startY: event.clientY,
    };
    activeRef.current = next;
    try {
      stableCaptureTarget.setPointerCapture?.(event.pointerId);
    } catch {
      // Lightweight test DOMs may not implement native pointer capture.
    }
  }

  function move(event: ReactPointerEvent<HTMLElement>) {
    const current = activeRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const position = { clientX: event.clientX, clientY: event.clientY };
    const active =
      current.active ||
      Math.hypot(
        position.clientX - current.startX,
        position.clientY - current.startY,
      ) >= SHEET_REORDER_POINTER_THRESHOLD_PX;
    const targetIndex = active
      ? optionsRef.current.resolveTarget(position)
      : null;
    const next: ActivePointer = {
      ...current,
      ...position,
      active,
      previewed: current.previewed || targetIndex !== null,
      resolvedTargetIndex: targetIndex,
    };
    activeRef.current = next;
    if (!active) return;
    event.preventDefault();
    setPointer(publicPointerState(next));
    optionsRef.current.onMove?.(position);
    if (targetIndex !== null) {
      optionsRef.current.onPreview(current.sourceId, targetIndex);
    }
  }

  function end(event: ReactPointerEvent<HTMLElement>) {
    const current = activeRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    const position = { clientX: event.clientX, clientY: event.clientY };
    if (!current.active) {
      const shouldActivate = optionsRef.current.validRelease(position);
      if (shouldActivate) {
        scheduleClickSuppression(current.sourceId);
      }
      finishPointer(current.pointerId, "none");
      if (shouldActivate) {
        optionsRef.current.onActivate(current.sourceId);
      }
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    scheduleClickSuppression(current.sourceId);
    const valid =
      current.previewed && optionsRef.current.validRelease(position);
    finishPointer(current.pointerId, valid ? "drop" : "cancel");
  }

  function cancel(event: ReactPointerEvent<HTMLElement>) {
    const current = activeRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    event.preventDefault();
    cancelActivePointer();
  }

  function lostCapture(event: ReactPointerEvent<HTMLElement>) {
    const current = activeRef.current;
    if (!current || current.pointerId !== event.pointerId) return;
    cancelActivePointer();
  }

  function consumeClickSuppression(sourceId: string): boolean {
    if (suppressedClickSourceRef.current !== sourceId) return false;
    clearClickSuppression();
    return true;
  }

  function refreshTarget() {
    const current = activeRef.current;
    if (!current?.active) return;
    const position = {
      clientX: current.clientX,
      clientY: current.clientY,
    };
    const targetIndex = optionsRef.current.resolveTarget(position);
    activeRef.current = {
      ...current,
      previewed: current.previewed || targetIndex !== null,
      resolvedTargetIndex: targetIndex,
    };
    if (
      targetIndex !== null &&
      targetIndex !== current.resolvedTargetIndex
    ) {
      optionsRef.current.onPreview(current.sourceId, targetIndex);
    }
  }

  return {
    begin,
    cancel,
    consumeClickSuppression,
    end,
    lostCapture,
    move,
    pointer,
    refreshTarget,
  };
}

function publicPointerState(
  active: ActivePointer,
): SheetReorderPointerState {
  return {
    active: active.active,
    clientX: active.clientX,
    clientY: active.clientY,
    sourceId: active.sourceId,
    startX: active.startX,
    startY: active.startY,
  };
}
