import { useEffect, useRef, useState } from "react";

import type {
  AlbumInformation,
  EditorProjection,
  ProjectIntent,
  ProjectedVisualDefaults,
} from "../domain/project";
import type {
  ProjectMutationOperation,
  ProjectMutationRunner,
} from "./useProjectMutationRunner";

interface ProjectMutationsInput {
  projection: EditorProjection;
  runProjectMutation: ProjectMutationRunner;
  onProjectionChange(projection: EditorProjection): void;
}

function messageFromError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export function useProjectMutations({
  projection,
  runProjectMutation,
  onProjectionChange,
}: ProjectMutationsInput) {
  const [message, setMessage] = useState<string | null>(null);
  const feedbackTokenRef = useRef(0);

  useEffect(() => {
    setMessage(null);
  }, [runProjectMutation, projection.state.projectId]);

  async function runWithErrorFeedback(
    operation: ProjectMutationOperation,
    cancelAfterPendingFailure = false,
  ) {
    const feedbackToken = feedbackTokenRef.current + 1;
    feedbackTokenRef.current = feedbackToken;
    setMessage(null);
    const outcome = await runProjectMutation.run(operation, {
      cancelAfterPendingFailure,
    });
    if (outcome.status === "completed") {
      onProjectionChange(outcome.projection);
    } else if (
      outcome.status === "failed" &&
      feedbackToken === feedbackTokenRef.current
    ) {
      setMessage(messageFromError(outcome.error));
    }
  }

  function applyIntent(intent: ProjectIntent) {
    return runWithErrorFeedback((port) =>
      port.apply(intent),
    );
  }

  function saveVisibleRevision() {
    const visibleRevision = projection.state.revision;
    return runWithErrorFeedback(
      async (port, latestProjection) => {
        const expectedRevision =
          latestProjection?.state.revision ?? visibleRevision;
        const result = await port.save(expectedRevision);
        return result.projection;
      },
      true,
    );
  }

  function runHistoryCommand(
    availability: "canUndo" | "canRedo",
    operation: "undo" | "redo",
  ) {
    return runWithErrorFeedback(
      (port, latestProjection) => {
        const effectiveProjection = latestProjection ?? projection;
        if (!effectiveProjection.state[availability]) {
          return Promise.resolve(effectiveProjection);
        }
        return port[operation]();
      },
      true,
    );
  }

  async function commitInteraction(intent: ProjectIntent) {
    setMessage(null);
    const outcome = await runProjectMutation.run((port) =>
      port.apply(intent),
    );
    if (outcome.status === "completed") {
      onProjectionChange(outcome.projection);
      return true;
    }
    if (outcome.status === "failed") {
      setMessage(messageFromError(outcome.error));
    }
    return false;
  }

  return {
    message,
    applyIntent,
    commitInteraction,
    applyAlbumInformation: (information: AlbumInformation) =>
      commitInteraction({
        kind: "setAlbumInformation",
        information,
      }),
    applyAlbumDesign: (visualDefaults: ProjectedVisualDefaults) =>
      commitInteraction({
        kind: "setVisualDefaults",
        visualDefaults,
      }),
    save: () => void saveVisibleRevision(),
    undo: () => void runHistoryCommand("canUndo", "undo"),
    redo: () => void runHistoryCommand("canRedo", "redo"),
    dismissFeedback: () => {
      setMessage(null);
    },
  };
}
