import { useCallback, useEffect, useRef, useState } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import {
  ProjectCloseError,
  type ProjectCloseChoice,
  type ProjectWindowPort,
} from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";

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
  onProjectionChange(projection: EditorProjection): void;
  onError(message: string): void;
}

function closeErrorMessage(error: unknown) {
  if (error instanceof ProjectCloseError || error instanceof Error) {
    return error.message;
  }
  return "Não foi possível concluir o fechamento do Projeto.";
}

export function useProjectCloseController({
  projectDialogPort,
  projectWindowPort,
  onProjectionChange,
  onError,
}: ProjectCloseControllerOptions) {
  const [phase, setPhase] = useState<ClosePhase>("idle");
  const phaseRef = useRef<ClosePhase>("idle");
  const dialogActionListener = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  const transition = useCallback((nextPhase: ClosePhase) => {
    phaseRef.current = nextPhase;
    setPhase(nextPhase);
  }, []);

  const presentConfirmation = useCallback(() => {
    transition("deciding");
    void projectDialogPort
      .present({ busy: false, kind: "projectCloseConfirmation" })
      .catch((error: unknown) => {
        if (phaseRef.current === "deciding") transition("idle");
        onError(closeErrorMessage(error));
      });
  }, [onError, projectDialogPort, transition]);

  const requestClose = useCallback(async () => {
    if (phaseRef.current !== "idle") return;
    transition("requesting");
    try {
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
  }, [onError, presentConfirmation, projectWindowPort, transition]);

  const resolveClose = useCallback(
    async (choice: ProjectCloseChoice) => {
      if (phaseRef.current !== "deciding") return;
      transition("resolving");
      void projectDialogPort
        .present({ busy: true, kind: "projectCloseConfirmation" })
        .catch((error: unknown) => onError(closeErrorMessage(error)));

      try {
        const resolution = await projectWindowPort.resolveClose(choice);
        if (resolution.kind === "cancelled") {
          onProjectionChange(resolution.projection);
          transition("idle");
          void projectDialogPort.dismiss().catch(() => undefined);
          return;
        }
        transition("terminal");
      } catch (error: unknown) {
        const message = closeErrorMessage(error);
        const indeterminate =
          error instanceof ProjectCloseError &&
          error.code === "save_state_indeterminate";
        transition(indeterminate ? "terminal" : "failed");
        void projectDialogPort
          .present({ kind: "projectCloseFailure", message })
          .catch(() => onError(message));
      }
    },
    [
      onError,
      onProjectionChange,
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
        void projectDialogPort.dismiss().catch(() => undefined);
        if (phaseRef.current === "failed") transition("idle");
        break;
      default:
        break;
    }
  };

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void projectDialogPort
      .onAction((action) => dialogActionListener.current(action))
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
  }, [onError, projectDialogPort]);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void projectWindowPort
      .onCloseRequested(() => {
        if (
          active &&
          phaseRef.current !== "terminal" &&
          phaseRef.current !== "resolving"
        ) {
          presentConfirmation();
        }
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
  }, [onError, presentConfirmation, projectWindowPort]);

  return {
    interactionBlocked: phase !== "idle",
    requestClose,
  };
}
