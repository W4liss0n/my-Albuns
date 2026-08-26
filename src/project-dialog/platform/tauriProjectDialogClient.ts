import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import {
  parseProjectDialogPresentation,
  toIpcProjectDialogAction,
} from "../../platform/projectDialogContract";
import type { ProjectDialogClient } from "../application/projectDialogClient";

export const PROJECT_DIALOG_PRESENTATION_EVENT =
  "myalbuns://project-dialog-presentation";

export const tauriProjectDialogClient: ProjectDialogClient = {
  onPresentation: async (listener) => {
    let hydrated = false;
    let presentationReceivedDuringHydration: ReturnType<
      typeof parseProjectDialogPresentation
    > = null;
    const unlisten = await listen<unknown>(
      PROJECT_DIALOG_PRESENTATION_EVENT,
      ({ payload }) => {
        const presentation = parseProjectDialogPresentation(payload);
        if (!presentation) return;
        if (hydrated) listener(presentation);
        else presentationReceivedDuringHydration = presentation;
      },
    );
    try {
      const current = parseProjectDialogPresentation(
        await invoke<unknown>("current_project_dialog_presentation"),
      );
      hydrated = true;
      const latest = presentationReceivedDuringHydration ?? current;
      if (latest) listener(latest);
      return unlisten;
    } catch (error) {
      unlisten();
      throw error;
    }
  },
  submit: (sessionId, action) =>
    invoke<void>("submit_project_dialog_action", {
      action: toIpcProjectDialogAction(action),
      sessionId,
    }),
};
