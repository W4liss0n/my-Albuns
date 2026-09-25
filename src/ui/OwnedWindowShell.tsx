import { type ReactNode, useLayoutEffect, useRef } from "react";

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
