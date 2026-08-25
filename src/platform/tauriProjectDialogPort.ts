import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";

export const PROJECT_DIALOG_ACTION_EVENT =
  "myalbuns://project-dialog-action";

const projectDialogActions = new Set<ProjectDialogAction>([
  "cancelAlbumInformation",
  "cancelExport",
  "cancelProjectClose",
  "confirmAlbumInformation",
  "discardAndClose",
  "dismissExport",
  "dismissProjectCloseFailure",
  "dismissProjectOperationFailure",
  "retryExport",
  "saveAndClose",
]);

function isProjectDialogAction(
  value: unknown,
): value is ProjectDialogAction {
  return (
    typeof value === "string" &&
    projectDialogActions.has(value as ProjectDialogAction)
  );
}

let dialogMutationQueue: Promise<void> = Promise.resolve();

function enqueueDialogMutation<T>(mutation: () => Promise<T>) {
  const result = dialogMutationQueue.then(mutation);
  dialogMutationQueue = result.then(
    () => undefined,
    () => undefined,
  );
  return result;
}

export const tauriProjectDialogPort: ProjectDialogPort = {
  dismiss: () =>
    enqueueDialogMutation(() => invoke<void>("dismiss_project_dialog")),
  onAction: (listener) =>
    listen<unknown>(PROJECT_DIALOG_ACTION_EVENT, ({ payload }) => {
      if (isProjectDialogAction(payload)) {
        listener(payload);
      }
    }),
  present: (state) =>
    enqueueDialogMutation(() =>
      invoke<void>("present_project_dialog", { state }),
    ),
};
