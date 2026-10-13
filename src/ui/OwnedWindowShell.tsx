import { type ReactNode, useEffect, useLayoutEffect, useRef } from "react";

import { ApplicationHeader } from "./ApplicationHeader";
import { useWindowControls } from "./WindowControlsContext";

interface OwnedWindowShellProps {
  children: ReactNode;
  controls?: "all" | "close" | "none";
  status?: string;
  context?: string;
  width?: number;
}

export function OwnedWindowShell({
  children,
  controls = "none",
  status,
  context,
  width,
}: OwnedWindowShellProps) {
  const shellRef = useRef<HTMLDivElement>(null);
  const windowControls = useWindowControls();

  useLayoutEffect(() => {
    const shell = shellRef.current;
    if (!shell) return;

    // Measure after the native adapter has prepared the available screen bounds.
    const measureHeight = () => Math.ceil(shell.getBoundingClientRect().height);
    const fitContent = () => {
      try {
        void Promise.resolve(windowControls.fitContent(measureHeight, width)).catch(
          () => undefined,
        );
      } catch {
        // Browser previews do not expose a native Tauri window.
      }
    };
    const observer = new ResizeObserver(fitContent);
    observer.observe(shell);
    fitContent();

    return () => observer.disconnect();
  }, [windowControls, width]);

  // Escape does what the window's close button does. Listening on the window,
  // after document listeners, leaves Escape to a surface or form that took it.
  useEffect(() => {
    if (controls === "none") return;
    const closeOnEscape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented || event.repeat) return;
      event.preventDefault();
      try {
        void Promise.resolve(windowControls.close()).catch(() => undefined);
      } catch {
        // Browser previews do not expose a native Tauri window.
      }
    };
    window.addEventListener("keydown", closeOnEscape);
    return () => window.removeEventListener("keydown", closeOnEscape);
  }, [controls, windowControls]);

  return (
    <div
      className="ui-owned-window-shell ui-chrome-selection-scope"
      ref={shellRef}
      style={width === undefined ? undefined : { width }}
    >
      {/* Dialogs never repeat the brand: the title bar names the flow, when there is one. */}
      <ApplicationHeader controls={controls} status={status} context={context} showBrand={false} />
      {children}
    </div>
  );
}
