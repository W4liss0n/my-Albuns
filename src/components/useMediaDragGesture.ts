import { useEffect, useRef, type PointerEvent as ReactPointerEvent } from "react";
import type { PointerDragThreshold } from "../application/projectPorts";
import type { MediaKind } from "../domain/project";

export interface MediaDrag {
  gestureId: number;
  mediaId: string;
  kind: MediaKind;
  x: number;
  y: number;
  shiftKey: boolean;
  phase: "dragging" | "drop";
}

export function useMediaDragGesture({ threshold, disabled, onChange }: {
  threshold: PointerDragThreshold | null;
  disabled: boolean;
  onChange(drag: MediaDrag | null): void;
}) {
  const current = useRef({ threshold, disabled, onChange });
  current.current = { threshold, disabled, onChange };
  const cleanup = useRef<(() => void) | null>(null);
  const sequence = useRef(0);
  const suppressClick = useRef(false);
  useEffect(() => () => cleanup.current?.(), []);
  useEffect(() => { if (disabled) cleanup.current?.(); }, [disabled]);
  return {
    suppressClick: () => suppressClick.current,
    start: (mediaId: string, kind: MediaKind, event: ReactPointerEvent<HTMLButtonElement>) => {
      const threshold = current.current.threshold;
      if (current.current.disabled || !threshold || event.button !== 0) return;
      cleanup.current?.();
      const origin = { x: event.clientX, y: event.clientY };
      const pointerId = event.pointerId;
      const gestureId = ++sequence.current;
      let active = false;
      let last: MediaDrag = { gestureId, mediaId, kind, ...origin, shiftKey: event.shiftKey, phase: "dragging" };
      const release = () => {
        document.removeEventListener("pointermove", move);
        document.removeEventListener("pointerup", up);
        document.removeEventListener("pointercancel", cancel);
        window.removeEventListener("keydown", key);
        window.removeEventListener("keyup", key);
        window.removeEventListener("blur", cancel);
        cleanup.current = null;
      };
      const cancel = () => {
        release();
        if (active) current.current.onChange(null);
        window.setTimeout(() => { suppressClick.current = false; }, 0);
      };
      const move = (next: PointerEvent) => {
        if (next.pointerId !== pointerId) return;
        if (!active && Math.abs(next.clientX - origin.x) < threshold.x && Math.abs(next.clientY - origin.y) < threshold.y) return;
        active = true;
        suppressClick.current = true;
        next.preventDefault();
        last = { ...last, x: next.clientX, y: next.clientY, shiftKey: next.shiftKey };
        current.current.onChange(last);
      };
      const up = (next: PointerEvent) => {
        if (next.pointerId !== pointerId) return;
        release();
        if (active) current.current.onChange({ ...last, x: next.clientX, y: next.clientY, shiftKey: next.shiftKey, phase: "drop" });
        window.setTimeout(() => { suppressClick.current = false; }, 0);
      };
      const key = (next: KeyboardEvent) => {
        if (next.key === "Escape") { next.preventDefault(); cancel(); }
        else if (next.key === "Shift" && active) {
          last = { ...last, shiftKey: next.type === "keydown" };
          current.current.onChange(last);
        }
      };
      cleanup.current = cancel;
      document.addEventListener("pointermove", move);
      document.addEventListener("pointerup", up);
      document.addEventListener("pointercancel", cancel);
      window.addEventListener("keydown", key);
      window.addEventListener("keyup", key);
      window.addEventListener("blur", cancel);
    },
  };
}
