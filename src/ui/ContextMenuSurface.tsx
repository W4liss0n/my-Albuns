import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
  type ReactNode,
} from "react";

import { useDismissableSurface } from "./useDismissableSurface";
import "./ContextMenuSurface.css";

const VIEWPORT_MARGIN_PX = 8;

interface ContextMenuSurfaceProps {
  label: string;
  position: { x: number; y: number };
  children: ReactNode;
  onDismiss(): void;
}

export function ContextMenuSurface({ label, position, children, onDismiss }: ContextMenuSurfaceProps) {
  const rootRef = useRef<HTMLDivElement>(null);
  const outsidePressRef = useRef(false);
  const [visiblePosition, setVisiblePosition] = useState(position);

  useLayoutEffect(() => {
    const keepInsideViewport = () => {
      const root = rootRef.current;
      if (!root) return;
      const bounds = root.getBoundingClientRect();
      const maximumX = Math.max(
        VIEWPORT_MARGIN_PX,
        window.innerWidth - bounds.width - VIEWPORT_MARGIN_PX,
      );
      const maximumY = Math.max(
        VIEWPORT_MARGIN_PX,
        window.innerHeight - bounds.height - VIEWPORT_MARGIN_PX,
      );
      const next = {
        x: Math.min(Math.max(position.x, VIEWPORT_MARGIN_PX), maximumX),
        y: Math.min(Math.max(position.y, VIEWPORT_MARGIN_PX), maximumY),
      };
      setVisiblePosition((current) =>
        current.x === next.x && current.y === next.y ? current : next,
      );
    };

    keepInsideViewport();
    window.addEventListener("resize", keepInsideViewport);
    return () => window.removeEventListener("resize", keepInsideViewport);
  }, [position.x, position.y]);

  useDismissableSurface({
    enabled: true,
    includeFocusOutside: true,
    rootRef,
    onDismiss: ({ reason, event }) => {
      if (reason === "escape") event.preventDefault();
      onDismiss();
    },
  });

  useEffect(() => {
    queueMicrotask(() => {
      rootRef.current
        ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
        ?.focus();
    });
  }, []);

  function handleKeyDown(event: ReactKeyboardEvent<HTMLDivElement>) {
    if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
    event.preventDefault();
    const items = Array.from(
      event.currentTarget.querySelectorAll<HTMLButtonElement>(
        "button:not(:disabled)",
      ),
    );
    if (items.length === 0) return;
    const current =
      event.target instanceof HTMLButtonElement
        ? items.indexOf(event.target)
        : -1;
    const delta = event.key === "ArrowDown" ? 1 : -1;
    const next =
      current < 0
        ? delta > 0
          ? 0
          : items.length - 1
        : (current + delta + items.length) % items.length;
    items[next]?.focus();
  }

  return (
    <>
      <div
        aria-hidden="true"
        className="ui-context-menu__dismiss-layer"
        onClick={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onDismiss();
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          event.stopPropagation();
          // A menu opened on right-button release can receive that gesture's
          // trailing contextmenu. Only a new outside press dismisses it.
          if (outsidePressRef.current) onDismiss();
        }}
        onPointerCancel={(event) => {
          event.preventDefault();
          event.stopPropagation();
          onDismiss();
        }}
        onPointerDown={(event) => {
          event.preventDefault();
          event.stopPropagation();
          outsidePressRef.current = true;
        }}
        onPointerUp={(event) => {
          event.preventDefault();
          event.stopPropagation();
        }}
      />
      <div
        ref={rootRef}
        aria-label={label}
        className="ui-floating-surface ui-context-menu"
        role="menu"
        style={{ left: visiblePosition.x, top: visiblePosition.y }}
        tabIndex={-1}
        onContextMenu={(event) => event.preventDefault()}
        onKeyDown={handleKeyDown}
      >
        {children}
      </div>
    </>
  );
}
