import type { ReactNode } from "react";

import { ApplicationHeader } from "./ApplicationHeader";

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
  return (
    <div className="ui-owned-window-shell">
      <ApplicationHeader controls={controls} status={status} />
      {children}
    </div>
  );
}
