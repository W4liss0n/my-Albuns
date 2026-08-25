import { useEffect, useRef, type RefObject } from "react";

export type SurfaceDismissal =
  | { reason: "escape"; event: KeyboardEvent }
  | { reason: "pointerOutside"; event: PointerEvent }
  | { reason: "focusOutside"; event: FocusEvent };

interface DismissableSurfaceOptions {
  enabled: boolean;
  includeFocusOutside?: boolean;
  rootRef: RefObject<HTMLElement | null>;
  onDismiss(dismissal: SurfaceDismissal): void;
}

/**
 * Owns only document-listener lifecycle, containment and dismissal reasons.
 * Focus restoration, propagation and submenu policy stay with each caller.
 */
export function useDismissableSurface({
  enabled,
  includeFocusOutside = false,
  rootRef,
  onDismiss,
}: DismissableSurfaceOptions) {
  const onDismissRef = useRef(onDismiss);
  onDismissRef.current = onDismiss;

  useEffect(() => {
    if (!enabled) return;
    const outside = (target: EventTarget | null) =>
      target instanceof Node && !rootRef.current?.contains(target);
    const onPointerDown = (event: PointerEvent) => {
      if (outside(event.target)) {
        onDismissRef.current({ reason: "pointerOutside", event });
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onDismissRef.current({ reason: "escape", event });
      }
    };
    const onFocusIn = (event: FocusEvent) => {
      if (outside(event.target)) {
        onDismissRef.current({ reason: "focusOutside", event });
      }
    };

    document.addEventListener("pointerdown", onPointerDown);
    document.addEventListener("keydown", onKeyDown);
    if (includeFocusOutside) document.addEventListener("focusin", onFocusIn);
    return () => {
      document.removeEventListener("pointerdown", onPointerDown);
      document.removeEventListener("keydown", onKeyDown);
      if (includeFocusOutside) {
        document.removeEventListener("focusin", onFocusIn);
      }
    };
  }, [enabled, includeFocusOutside, rootRef]);
}
