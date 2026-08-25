import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import {
  parseProjectDialogState,
  toIpcProjectDialogAction,
} from "../../platform/projectDialogContract";
import type { ProjectDialogClient } from "../application/projectDialogClient";

export const PROJECT_DIALOG_STATE_EVENT =
  "myalbuns://project-dialog-state";

export const tauriProjectDialogClient: ProjectDialogClient = {
  onState: async (listener) => {
    let hydrated = false;
    let stateReceivedDuringHydration: ReturnType<
      typeof parseProjectDialogState
    > = null;
    const unlisten = await listen<unknown>(
      PROJECT_DIALOG_STATE_EVENT,
      ({ payload }) => {
        const state = parseProjectDialogState(payload);
        if (!state) return;
        if (hydrated) listener(state);
        else stateReceivedDuringHydration = state;
      },
    );
    try {
      const current = parseProjectDialogState(
        await invoke<unknown>("current_project_dialog_state"),
      );
      hydrated = true;
      const latest = stateReceivedDuringHydration ?? current;
      if (latest) listener(latest);
      return unlisten;
    } catch (error) {
      unlisten();
      throw error;
    }
  },
  submit: (action) =>
    invoke<void>("submit_project_dialog_action", {
      action: toIpcProjectDialogAction(action),
    }),
};
