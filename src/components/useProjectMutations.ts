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
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const feedbackTokenRef = useRef(0);

  useEffect(() => {
    setBusy(null);
    setMessage(null);
  }, [runProjectMutation, projection.state.projectId]);

  async function runWithGlobalFeedback(
    label: string,
    operation: ProjectMutationOperation,
  ) {
    const feedbackToken = feedbackTokenRef.current + 1;
    feedbackTokenRef.current = feedbackToken;
    setBusy(label);
    setMessage(null);
    const outcome = await runProjectMutation(operation);
    if (outcome.status === "completed") {
      onProjectionChange(outcome.projection);
    } else if (
      outcome.status === "failed" &&
      feedbackToken === feedbackTokenRef.current
    ) {
      setMessage(messageFromError(outcome.error));
    }
    if (feedbackToken === feedbackTokenRef.current) {
      setBusy(null);
    }
  }

  function applyWithStatus(intent: ProjectIntent) {
    return runWithGlobalFeedback("Aplicando alteração", (port) =>
      port.apply(intent),
    );
  }

  function saveVisibleRevision() {
    const expectedRevision = projection.state.revision;
    return runWithGlobalFeedback("Salvando", async (port) => {
      const result = await port.save(expectedRevision);
      return result.projection;
    });
  }

  async function commitInteraction(intent: ProjectIntent) {
    setMessage(null);
    const outcome = await runProjectMutation((port) =>
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
    busy,
    message,
    applyWithStatus,
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
    undo: () =>
      void runWithGlobalFeedback("Desfazendo", (port) =>
        port.undo(),
      ),
    redo: () =>
      void runWithGlobalFeedback("Refazendo", (port) =>
        port.redo(),
      ),
    dismissFeedback: () => {
      setMessage(null);
    },
  };
}
