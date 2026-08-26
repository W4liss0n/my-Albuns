import { invoke } from "@tauri-apps/api/core";

import type { ProjectFailureDialogPort } from "../application/globalProjectPort";

export const tauriProjectFailureDialogPort: ProjectFailureDialogPort = {
  present: async ({ context, error }) => {
    try {
      await invoke<void>("show_project_failure_dialog", { context, error });
    } catch {
      // Logging and the safe fallback remain owned by the native host.
    }
  },
};
