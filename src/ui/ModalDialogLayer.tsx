import {
  useLayoutEffect,
  useRef,
  type ReactNode,
  type RefObject,
} from "react";

import "./ModalDialogLayer.css";

const FOCUSABLE_SELECTOR = [
  "button:not(:disabled)",
  "[href]",
  "input:not(:disabled)",
  "select:not(:disabled)",
  "textarea:not(:disabled)",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

interface ModalDialogLayerProps {
  children: ReactNode;
  focusKey: string;
  initialFocusRef?: RefObject<HTMLElement | null>;
  onEscape(): void;
  owner: string;
}

/**
 * Supplies in-owner modal lifecycle around the standard dialog components.
 * Decisions and outside-click policy remain with the owning feature.
 */
export function ModalDialogLayer({
  children,
  focusKey,
  initialFocusRef,
  onEscape,
  owner,
}: ModalDialogLayerProps) {
  const layerRef = useRef<HTMLDivElement>(null);
  const restoreFocusRef = useRef<HTMLElement | null>(null);

  const focusInitialTarget = () => {
    const layer = layerRef.current;
    const preferredTarget = initialFocusRef?.current;
    const initialTarget =
      (preferredTarget &&
        layer?.contains(preferredTarget) &&
        preferredTarget.matches(FOCUSABLE_SELECTOR)
        ? preferredTarget
        : null) ??
      layer?.querySelector<HTMLElement>(FOCUSABLE_SELECTOR) ??
      layer;
    initialTarget?.focus({ preventScroll: true });
  };

  useLayoutEffect(() => {
    restoreFocusRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;
    const keepFocusInside = (event: FocusEvent) => {
      if (
        event.target instanceof Node &&
        !layerRef.current?.contains(event.target)
      ) {
        focusInitialTarget();
      }
    };
    document.addEventListener("focusin", keepFocusInside);
    return () => {
      document.removeEventListener("focusin", keepFocusInside);
      const restoreTarget = restoreFocusRef.current;
      if (restoreTarget?.isConnected) {
        restoreTarget.focus({ preventScroll: true });
      }
    };
  }, []);

  useLayoutEffect(() => {
    focusInitialTarget();
  }, [focusKey, initialFocusRef]);

  const focusableElements = () =>
    Array.from(
      layerRef.current?.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR) ?? [],
    ).filter((element) => !element.hidden);

  return (
    <div
      className="ui-modal-dialog-layer"
      data-modal-owner={owner}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          onEscape();
          return;
        }
        if (event.key !== "Tab") return;

        const focusable = focusableElements();
        if (focusable.length === 0) {
          event.preventDefault();
          layerRef.current?.focus({ preventScroll: true });
          return;
        }
        const first = focusable[0];
        const last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) {
          event.preventDefault();
          last.focus({ preventScroll: true });
        } else if (!event.shiftKey && document.activeElement === last) {
          event.preventDefault();
          first.focus({ preventScroll: true });
        }
      }}
      onPointerDown={(event) => {
        if (event.target === event.currentTarget) {
          event.preventDefault();
        }
      }}
      ref={layerRef}
      tabIndex={-1}
    >
      <div className="ui-modal-dialog-layer__dialog">{children}</div>
    </div>
  );
}
