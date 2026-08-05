import { useCallback, useEffect, useState } from "react";

import {
  ProjectCloseError,
  type ProjectCloseChoice,
  type ProjectWindowPort,
} from "../application/projectPorts";
import type { EditorProjection } from "../domain/project";

type ClosePhase = "idle" | "requesting" | "deciding" | "resolving" | "terminal";

interface ProjectCloseControllerOptions {
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
  projectWindowPort,
  onProjectionChange,
  onError,
}: ProjectCloseControllerOptions) {
  const [phase, setPhase] = useState<ClosePhase>("idle");

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;

    projectWindowPort
      .onCloseRequested(() => {
        if (active) {
          setPhase((current) =>
            current === "terminal" || current === "resolving"
              ? current
              : "deciding",
          );
        }
      })
      .then((registeredUnsubscribe) => {
        if (active) {
          unsubscribe = registeredUnsubscribe;
        } else {
          registeredUnsubscribe();
        }
      })
      .catch((error: unknown) => {
        if (active) {
          onError(closeErrorMessage(error));
        }
      });

    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [onError, projectWindowPort]);

  const requestClose = useCallback(async () => {
    if (phase !== "idle") return;
    setPhase("requesting");
    try {
      const outcome = await projectWindowPort.requestClose();
      setPhase(
        outcome.kind === "confirmationRequired" ? "deciding" : "terminal",
      );
    } catch (error: unknown) {
      setPhase("idle");
      onError(closeErrorMessage(error));
    }
  }, [onError, phase, projectWindowPort]);

  const resolveClose = useCallback(
    async (choice: ProjectCloseChoice) => {
      if (phase !== "deciding") return;
      setPhase("resolving");
      try {
        const resolution = await projectWindowPort.resolveClose(choice);
        if (resolution.kind === "cancelled") {
          onProjectionChange(resolution.projection);
          setPhase("idle");
          return;
        }
        setPhase("terminal");
      } catch (error: unknown) {
        const indeterminate =
          error instanceof ProjectCloseError &&
          error.code === "save_state_indeterminate";
        setPhase(indeterminate ? "terminal" : "idle");
        onError(closeErrorMessage(error));
      }
    },
    [onError, onProjectionChange, phase, projectWindowPort],
  );

  return {
    confirmationVisible: phase === "deciding" || phase === "resolving",
    interactionBlocked: phase !== "idle",
    resolving: phase === "resolving",
    requestClose,
    resolveClose,
  };
}
