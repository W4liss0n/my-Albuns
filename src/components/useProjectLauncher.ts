import { useCallback, useRef } from "react";
import type { ProjectLauncher } from "../application/projectLauncher";

export function useProjectLauncher(
  launcher: ProjectLauncher | undefined,
  disabled: boolean,
  reportError: (message: string) => void,
) {
  const pending = useRef(false);
  const launch = useCallback(async (action: keyof ProjectLauncher) => {
    if (!launcher || disabled || pending.current) return;
    pending.current = true;
    try {
      await launcher[action]();
    } catch (error) {
      reportError(String(error));
    } finally {
      pending.current = false;
    }
  }, [disabled, launcher, reportError]);
  return {
    newProject: launcher && !disabled ? () => { void launch("newProject"); } : undefined,
    openProject: launcher && !disabled ? () => { void launch("openProject"); } : undefined,
  };
}
