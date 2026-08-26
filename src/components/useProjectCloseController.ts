import { useCallback, useEffect, useRef, useState } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";
import {
  ProjectCloseError,
  type ProjectCloseChoice,
  type ProjectWindowPort,
} from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";
import type { ProjectMutationOutcome } from "./useProjectMutationRunner";

type ClosePhase =
  | "idle"
  | "requesting"
  | "deciding"
  | "resolving"
  | "failed"
  | "terminal";

interface ProjectCloseControllerOptions {
  projectDialogPort: ProjectDialogPort;
  projectWindowPort: ProjectWindowPort;
  waitForPendingMutations(): Promise<ProjectMutationOutcome | null>;
  onProjectionChange(projection: EditorProjection): void;
  onError(message: string): void;
}

function closeErrorMessage(error: unknown) {
  if (error instanceof ProjectCloseError || error instanceof Error) {
    return error.message;
  }
  return "Não foi possível concluir o fechamento do Projeto.";
}

function hasClosePhase(
  phaseRef: { readonly current: ClosePhase },
  expected: ClosePhase,
) {
  return phaseRef.current === expected;
}

export function useProjectCloseController({
  projectDialogPort,
  projectWindowPort,
  waitForPendingMutations,
  onProjectionChange,
  onError,
}: ProjectCloseControllerOptions) {
  const [phase, setPhase] = useState<ClosePhase>("idle");
  const phaseRef = useRef<ClosePhase>("idle");
  const dialogSessionRef = useRef<ProjectDialogSession | null>(null);
  const dialogActionListener = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  const transition = useCallback((nextPhase: ClosePhase) => {
    phaseRef.current = nextPhase;
    setPhase(nextPhase);
  }, []);

  const dismissDialog = useCallback(() => {
    const session = dialogSessionRef.current;
    dialogSessionRef.current = null;
    void session?.dismiss().catch(() => undefined);
  }, []);

  const presentConfirmation = useCallback(() => {
    transition("deciding");
    const session = projectDialogPort.acquire(
      (action) => dialogActionListener.current(action),
    );
    dialogSessionRef.current = session;
    void session
      .present({ busy: false, kind: "projectCloseConfirmation" })
      .catch(async (error: unknown) => {
        if (
          dialogSessionRef.current !== session ||
          phaseRef.current !== "deciding"
        ) {
          return;
        }
        dialogSessionRef.current = null;
        await session.dismiss().catch(() => undefined);
        try {
          const resolution = await projectWindowPort.resolveClose("cancel");
          if (resolution.kind === "cancelled") {
            onProjectionChange(resolution.projection);
            transition("idle");
          } else {
            transition("terminal");
          }
          onError(closeErrorMessage(error));
        } catch (releaseError: unknown) {
          transition("terminal");
          onError(closeErrorMessage(releaseError));
        }
      });
  }, [
    onError,
    onProjectionChange,
    projectDialogPort,
    projectWindowPort,
    transition,
  ]);

  const requestClose = useCallback(async () => {
    if (phaseRef.current !== "idle") return;
    transition("requesting");
    try {
      const pendingOutcome = await waitForPendingMutations();
      if (!hasClosePhase(phaseRef, "requesting")) return;
      if (
        pendingOutcome?.status === "failed" ||
        pendingOutcome?.status === "obsolete"
      ) {
        transition("idle");
        return;
      }
      const outcome = await projectWindowPort.requestClose();
      if (outcome.kind === "confirmationRequired") {
        presentConfirmation();
      } else {
        transition("terminal");
      }
    } catch (error: unknown) {
      transition("idle");
      onError(closeErrorMessage(error));
    }
  }, [
    onError,
    presentConfirmation,
    projectWindowPort,
    transition,
    waitForPendingMutations,
  ]);

  const resolveClose = useCallback(
    async (choice: ProjectCloseChoice) => {
      if (phaseRef.current !== "deciding") return;
      transition("resolving");
      let dialogProjection = Promise.resolve();
      if (choice !== "cancel") {
        const session = dialogSessionRef.current;
        dialogProjection =
          session
            ?.present({ busy: true, kind: "projectCloseConfirmation" })
            .catch((error: unknown) => {
              if (dialogSessionRef.current === session) {
                dialogSessionRef.current = null;
                void session.dismiss().catch(() => undefined);
              }
              onError(closeErrorMessage(error));
            }) ?? Promise.resolve();
      }

      try {
        const resolution = await projectWindowPort.resolveClose(choice);
        if (resolution.kind === "cancelled") {
          onProjectionChange(resolution.projection);
          transition("idle");
          dismissDialog();
          return;
        }
        transition("terminal");
      } catch (error: unknown) {
        await dialogProjection;
        const message = closeErrorMessage(error);
        const indeterminate =
          error instanceof ProjectCloseError &&
          error.code === "save_state_indeterminate";
        transition(indeterminate ? "terminal" : "failed");
        const session =
          dialogSessionRef.current ??
          projectDialogPort.acquire(
            (action) => dialogActionListener.current(action),
          );
        dialogSessionRef.current = session;
        void session
          .present({ kind: "projectCloseFailure", message })
          .catch(() => {
            if (dialogSessionRef.current === session) dismissDialog();
            if (!indeterminate && phaseRef.current === "failed") {
              transition("idle");
            }
            onError(message);
          });
      }
    },
    [
      onError,
      onProjectionChange,
      dismissDialog,
      projectDialogPort,
      projectWindowPort,
      transition,
    ],
  );

  dialogActionListener.current = (action) => {
    switch (action) {
      case "cancelProjectClose":
        void resolveClose("cancel");
        break;
      case "discardAndClose":
        void resolveClose("discardAndClose");
        break;
      case "saveAndClose":
        void resolveClose("saveAndClose");
        break;
      case "dismissProjectCloseFailure":
        dismissDialog();
        if (phaseRef.current === "failed") transition("idle");
        break;
      default:
        break;
    }
  };

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    const releaseNativeClose = async () => {
      try {
        const resolution = await projectWindowPort.resolveClose("cancel");
        if (!active || !hasClosePhase(phaseRef, "requesting")) return;
        if (resolution.kind === "cancelled") {
          onProjectionChange(resolution.projection);
          transition("idle");
        } else {
          transition("terminal");
        }
      } catch (error: unknown) {
        if (!active || !hasClosePhase(phaseRef, "requesting")) return;
        transition("idle");
        onError(closeErrorMessage(error));
      }
    };
    void projectWindowPort
      .onCloseRequested(() => {
        if (!active || phaseRef.current !== "idle") return;
        transition("requesting");
        void waitForPendingMutations().then((pendingOutcome) => {
          if (!active || !hasClosePhase(phaseRef, "requesting")) return;
          if (
            pendingOutcome?.status === "failed" ||
            pendingOutcome?.status === "obsolete"
          ) {
            void releaseNativeClose();
            return;
          }
          presentConfirmation();
        });
      })
      .then((registeredUnsubscribe) => {
        if (active) unsubscribe = registeredUnsubscribe;
        else registeredUnsubscribe();
      })
      .catch((error: unknown) => {
        if (active) onError(closeErrorMessage(error));
      });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [
    onError,
    onProjectionChange,
    presentConfirmation,
    projectWindowPort,
    transition,
    waitForPendingMutations,
  ]);

  useEffect(
    () => () => {
      const session = dialogSessionRef.current;
      dialogSessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
    },
    [],
  );

  return {
    interactionBlocked: phase !== "idle",
    requestClose,
  };
}
