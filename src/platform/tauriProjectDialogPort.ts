import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import {
  parseProjectDialogAction,
  toIpcProjectDialogState,
} from "./projectDialogContract";

export const PROJECT_DIALOG_ACTION_EVENT =
  "myalbuns://project-dialog-action";

let dialogMutationQueue: Promise<void> = Promise.resolve();
const actionListeners = new Set<
  (action: ProjectDialogAction) => void
>();
let actionSubscription:
  | Promise<() => void>
  | undefined;
let unlistenFromActions: (() => void) | undefined;

function enqueueDialogMutation<T>(mutation: () => Promise<T>) {
  const result = dialogMutationQueue.then(mutation);
  dialogMutationQueue = result.then(
    () => undefined,
    () => undefined,
  );
  return result;
}

async function ensureActionSubscription() {
  actionSubscription ??= listen<unknown>(
    PROJECT_DIALOG_ACTION_EVENT,
    ({ payload }) => {
      const action = parseProjectDialogAction(payload);
      if (!action) return;
      for (const listener of actionListeners) listener(action);
    },
  ).then(
    (unlisten) => {
      unlistenFromActions = unlisten;
      return unlisten;
    },
    (error: unknown) => {
      actionSubscription = undefined;
      throw error;
    },
  );
  await actionSubscription;
}

function releaseActionSubscriptionIfUnused() {
  if (actionListeners.size > 0 || !unlistenFromActions) return;
  unlistenFromActions();
  unlistenFromActions = undefined;
  actionSubscription = undefined;
}

export const tauriProjectDialogPort: ProjectDialogPort = {
  dismiss: () =>
    enqueueDialogMutation(() => invoke<void>("dismiss_project_dialog")),
  onAction: async (listener) => {
    actionListeners.add(listener);
    try {
      await ensureActionSubscription();
    } catch (error: unknown) {
      actionListeners.delete(listener);
      throw error;
    }

    let active = true;
    return () => {
      if (!active) return;
      active = false;
      actionListeners.delete(listener);
      releaseActionSubscriptionIfUnused();
    };
  },
  present: (state) =>
    enqueueDialogMutation(() =>
      invoke<void>("present_project_dialog", {
        state: toIpcProjectDialogState(state),
      }),
    ),
};
