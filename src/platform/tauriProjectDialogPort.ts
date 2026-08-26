import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
  ProjectDialogState,
} from "../application/projectDialogPort";
import {
  parseProjectDialogActionEvent,
  toIpcProjectDialogState,
} from "./projectDialogContract";

export const PROJECT_DIALOG_ACTION_EVENT =
  "myalbuns://project-dialog-action";

type SessionStatus =
  | "new"
  | "queued"
  | "presenting"
  | "active"
  | "closing"
  | "closed";

interface PresentationWaiter {
  reject(error: unknown): void;
  resolve(): void;
}

interface OwnedDialogSession {
  id: string;
  listener(action: ProjectDialogAction): void;
  presentedVersion: number;
  state: ProjectDialogState | null;
  status: SessionStatus;
  version: number;
  waiters: PresentationWaiter[];
}

export function createTauriProjectDialogPort(): ProjectDialogPort {
  let nextSessionId = 0;
  let activeSession: OwnedDialogSession | null = null;
  const waitingSessions: OwnedDialogSession[] = [];
  let dialogMutationQueue: Promise<void> = Promise.resolve();
  let actionSubscription: Promise<void> | null = null;
  let terminalFailure: { error: unknown } | null = null;

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
        const event = parseProjectDialogActionEvent(payload);
        if (
          !event ||
          activeSession?.id !== event.sessionId ||
          (activeSession.status !== "active" &&
            !(
              activeSession.status === "presenting" &&
              activeSession.presentedVersion > 0
            ))
        ) {
          return;
        }
        activeSession.listener(event.action);
      },
    ).then(
      () => undefined,
      (error: unknown) => {
        actionSubscription = null;
        throw error;
      },
    );
    await actionSubscription;
  }

  function settlePresentation(
    session: OwnedDialogSession,
    outcome: { error: unknown } | { success: true },
  ) {
    const waiters = session.waiters.splice(0);
    for (const waiter of waiters) {
      if ("error" in outcome) waiter.reject(outcome.error);
      else waiter.resolve();
    }
  }

  function failRemainingSessions(
    error: unknown,
    failedSession: OwnedDialogSession,
  ) {
    terminalFailure = { error };
    failedSession.status = "closed";
    settlePresentation(failedSession, { error });
    if (activeSession === failedSession) activeSession = null;
    for (const waiting of waitingSessions.splice(0)) {
      waiting.status = "closed";
      settlePresentation(waiting, { error });
    }
  }

  async function releaseFailedPresentation(
    session: OwnedDialogSession,
    presentationError: unknown,
  ) {
    try {
      await enqueueDialogMutation(() =>
        invoke<void>("dismiss_project_dialog", {
          sessionId: session.id,
        }),
      );
    } catch (dismissalError: unknown) {
      failRemainingSessions(dismissalError, session);
      return;
    }
    session.status = "closed";
    if (activeSession === session) activeSession = null;
    settlePresentation(session, { error: presentationError });
    activateNextSession();
  }

  function synchronizeSession(session: OwnedDialogSession) {
    void (async () => {
      try {
        await ensureActionSubscription();
        while (
          session.status === "presenting" &&
          session.presentedVersion < session.version
        ) {
          const version = session.version;
          const state = session.state!;
          await enqueueDialogMutation(() =>
            invoke<void>("present_project_dialog", {
              sessionId: session.id,
              state: toIpcProjectDialogState(state),
            }),
          );
          session.presentedVersion = version;
        }
      } catch (presentationError: unknown) {
        await releaseFailedPresentation(session, presentationError);
        return;
      }
      if (session.status === "presenting") {
        session.status = "active";
        settlePresentation(session, { success: true });
      }
    })();
  }

  function activateNextSession() {
    if (activeSession) return;
    const next = waitingSessions.shift();
    if (!next) return;
    if (next.status !== "queued" || !next.state) {
      activateNextSession();
      return;
    }

    activeSession = next;
    next.status = "presenting";
    synchronizeSession(next);
  }

  function acquire(
    listener: (action: ProjectDialogAction) => void,
  ): ProjectDialogSession {
    const owned: OwnedDialogSession = {
      id: `project-dialog-session-${++nextSessionId}`,
      listener,
      presentedVersion: 0,
      state: null,
      status: "new",
      version: 0,
      waiters: [],
    };

    return {
      dismiss: async () => {
        if (owned.status === "closed") return;
        if (owned.status === "new") {
          owned.status = "closed";
          return;
        }
        if (owned.status === "queued") {
          const index = waitingSessions.indexOf(owned);
          if (index >= 0) waitingSessions.splice(index, 1);
          owned.status = "closed";
          settlePresentation(owned, { success: true });
          return;
        }
        if (owned.status === "closing") return;

        owned.status = "closing";
        try {
          await enqueueDialogMutation(() =>
            invoke<void>("dismiss_project_dialog", {
              sessionId: owned.id,
            }),
          );
          owned.status = "closed";
          if (activeSession === owned) activeSession = null;
          settlePresentation(owned, { success: true });
          activateNextSession();
        } catch (error: unknown) {
          failRemainingSessions(error, owned);
          throw error;
        }
      },
      present: (state) => {
        if (owned.status === "closed" || owned.status === "closing") {
          return Promise.resolve();
        }
        if (terminalFailure) {
          return Promise.reject(terminalFailure.error);
        }
        owned.state = state;
        ++owned.version;

        const presented = new Promise<void>((resolve, reject) => {
          owned.waiters.push({ reject, resolve });
        });
        if (owned.status === "new") {
          owned.status = "queued";
          waitingSessions.push(owned);
          activateNextSession();
        } else if (owned.status === "active") {
          owned.status = "presenting";
          synchronizeSession(owned);
        }
        return presented;
      },
    };
  }

  return { acquire };
}

export const tauriProjectDialogPort = createTauriProjectDialogPort();
