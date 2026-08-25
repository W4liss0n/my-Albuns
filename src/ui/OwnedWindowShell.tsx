import { type ReactNode, useLayoutEffect, useRef } from "react";

import { ApplicationHeader } from "./ApplicationHeader";
import { useWindowControls } from "./WindowControlsContext";

interface OwnedWindowShellProps {
  children: ReactNode;
  controls?: "all" | "close" | "none";
  status?: string;
}

export function OwnedWindowShell({
  children,
  controls = "none",
  status,
}: OwnedWindowShellProps) {
  const shellRef = useRef<HTMLDivElement>(null);
  const windowControls = useWindowControls();

  useLayoutEffect(() => {
    const shell = shellRef.current;
    if (!shell) return;

    let lastHeight = 0;
    const fitContent = () => {
      const height = Math.ceil(shell.scrollHeight);
      if (height <= 0 || height === lastHeight) return;
      lastHeight = height;
      try {
        void Promise.resolve(windowControls.fitContent(height)).catch(
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
  }, [windowControls]);

  return (
    <div className="ui-owned-window-shell" ref={shellRef}>
      <ApplicationHeader controls={controls} status={status} />
      {children}
    </div>
  );
}
