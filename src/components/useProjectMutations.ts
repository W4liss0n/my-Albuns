import { useEffect, useRef, useState } from "react";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import {
  materializeProjectIntent,
  type AlbumDesignProjectDraft,
  type AlbumInformationProjectDraft,
  type ProjectSettingsDraft,
} from "../application/projectSettingsDraft";
import { materializeSheetReorderTarget } from "../application/sheetStructure";
import {
  albumInformationReviewEquals,
  albumInformationReviewHasChanges,
  createAlbumInformationReview,
  type AlbumInformationCommitResult,
  type AlbumInformationReview,
} from "../application/albumInformationReview";
import type {
  ProjectMutationOperation,
  ProjectMutationRunner,
} from "./useProjectMutationRunner";

interface ProjectMutationsInput {
  projection: EditorProjection;
  runProjectMutation: ProjectMutationRunner;
  onProjectionChange(projection: EditorProjection): void;
  onAffectedFrame(frameId: string): void;
  onAffectedSheet(sheetId: string): void;
  onSaveAsBarrierChange?(active: boolean): void;
}

function messageFromError(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export function useProjectMutations({
  projection,
  runProjectMutation,
  onProjectionChange,
  onAffectedFrame,
  onAffectedSheet,
  onSaveAsBarrierChange,
}: ProjectMutationsInput) {
  const [message, setMessage] = useState<string | null>(null);
  const feedbackTokenRef = useRef(0);
  const saveAsBarrierRef = useRef(false);

  useEffect(() => {
    setMessage(null);
  }, [runProjectMutation, projection.state.projectId]);

  useEffect(() => {
    saveAsBarrierRef.current = false;
    onSaveAsBarrierChange?.(false);
  }, [onSaveAsBarrierChange, projection.state.projectId]);

  function releaseSaveAsBarrier() {
    saveAsBarrierRef.current = false;
    onSaveAsBarrierChange?.(false);
  }

  async function runWithErrorFeedback(
    operation: ProjectMutationOperation,
    cancelAfterPendingFailure = false,
  ) {
    if (saveAsBarrierRef.current) return false;
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
    return outcome.status === "completed";
  }

  function applyIntent(intent: ProjectIntent) {
    const capturedProjection = projection;
    return runWithErrorFeedback((port, latestProjection) =>
      port.apply(
        materializeProjectIntent(
          intent,
          capturedProjection,
          latestProjection ?? capturedProjection,
        ),
      ),
    );
  }

  async function applyWithOutcome(intent: ProjectIntent) {
    const capturedProjection = projection;
    let affectedFrameId: string | null = null;
    let affectedSheetId: string | null = null;
    let reorderCancelled = false;
    const completed = await runWithErrorFeedback(
      async (port, latestProjection) => {
        const effectiveProjection = latestProjection ?? capturedProjection;
        let materializedIntent = intent;
        if (intent.kind === "reorderSheet") {
          const materializedTarget = materializeSheetReorderTarget(
            capturedProjection.state.album.sheets,
            effectiveProjection.state.album.sheets,
            intent.sheetId,
            intent.targetIndex,
          );
          if (materializedTarget === null) {
            reorderCancelled = true;
            return effectiveProjection;
          }
          materializedIntent = {
            ...intent,
            targetIndex: materializedTarget,
          };
        }
        const result = await port.applyWithOutcome(materializedIntent);
        affectedFrameId = result.affectedFrameId;
        affectedSheetId = result.affectedSheetId;
        return result.projection;
      },
    );
    if (reorderCancelled) return false;
    if (completed && affectedFrameId) onAffectedFrame(affectedFrameId);
    if (completed && affectedSheetId) onAffectedSheet(affectedSheetId);
    return completed;
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

  function saveVisibleRevisionAs() {
    if (saveAsBarrierRef.current) return Promise.resolve();
    saveAsBarrierRef.current = true;
    onSaveAsBarrierChange?.(true);
    const visibleRevision = projection.state.revision;
    const feedbackToken = feedbackTokenRef.current + 1;
    feedbackTokenRef.current = feedbackToken;
    setMessage(null);
    let savedAs = false;
    return runProjectMutation
      .run(
        async (port, latestProjection) => {
          const expectedRevision =
            latestProjection?.state.revision ?? visibleRevision;
          const result = await port.saveAs(expectedRevision);
          savedAs = result.outcome.kind === "savedAs";
          return result.projection;
        },
        { cancelAfterPendingFailure: true },
      )
      .then((outcome) => {
        try {
          if (outcome.status === "completed") {
            onProjectionChange(outcome.projection);
          } else if (
            outcome.status === "failed" &&
            feedbackToken === feedbackTokenRef.current
          ) {
            setMessage(messageFromError(outcome.error));
          }
        } finally {
          if (!savedAs) releaseSaveAsBarrier();
        }
      });
  }

  async function commitInteraction(intent: ProjectIntent) {
    const capturedProjection = projection;
    return commitMutation((port, latestProjection) =>
      port.apply(
        materializeProjectIntent(
          intent,
          capturedProjection,
          latestProjection ?? capturedProjection,
        ),
      ),
    );
  }

  function commitProjectSettingsDraft<Value, Delta>(
    draft: ProjectSettingsDraft<Value, Delta>,
  ) {
    return commitMutation((port, latestProjection) => {
      const effectiveProjection = latestProjection ?? projection;
      const materialized = draft.materializeAgainst(effectiveProjection);
      return materialized.changed
        ? port.apply(materialized.intent)
        : Promise.resolve(effectiveProjection);
    });
  }

  async function commitAlbumInformation(
    draft: AlbumInformationProjectDraft,
    confirmedReview: AlbumInformationReview,
  ): Promise<AlbumInformationCommitResult> {
    if (saveAsBarrierRef.current) return { kind: "rejected" };
    setMessage(null);
    let currentReview: AlbumInformationReview | null = null;
    let validationRejected = false;
    let reviewRequired = false;
    let applyRequested = false;
    let intentAlreadySatisfied = false;
    const outcome = await runProjectMutation.run(
      async (port, latestProjection) => {
        const effectiveProjection = latestProjection ?? projection;
        const materialized = draft.materializeAgainst(effectiveProjection);
        const validation = await port.validateAlbumInformation(
          materialized.value,
        );
        if (validation.errors.length > 0 || !validation.impact) {
          validationRejected = true;
          return effectiveProjection;
        }
        currentReview = createAlbumInformationReview(
          materialized.baseline,
          materialized.value,
          validation.impact,
        );
        if (!albumInformationReviewHasChanges(currentReview)) {
          intentAlreadySatisfied = true;
          return effectiveProjection;
        }
        if (!albumInformationReviewEquals(confirmedReview, currentReview)) {
          reviewRequired = true;
          return effectiveProjection;
        }
        applyRequested = true;
        return port.apply(materialized.intent);
      },
    );

    if (outcome.status === "completed") {
      if (validationRejected) {
        setMessage(
          "As Informações do Álbum mudaram enquanto a confirmação estava aberta e precisam ser revistas antes de Aplicar.",
        );
        return { kind: "rejected" };
      }
      if (reviewRequired && currentReview) {
        return { kind: "reviewRequired", review: currentReview };
      }
      if (intentAlreadySatisfied) return { kind: "completed" };
      if (applyRequested) {
        onProjectionChange(outcome.projection);
        return { kind: "completed" };
      }
    } else if (outcome.status === "failed") {
      setMessage(messageFromError(outcome.error));
      return { kind: "rejected" };
    }
    return { kind: "rejected" };
  }

  async function commitMutation(operation: ProjectMutationOperation) {
    if (saveAsBarrierRef.current) return false;
    setMessage(null);
    const outcome = await runProjectMutation.run(operation);
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
    applyAlbumInformation: commitAlbumInformation,
    applyAlbumDesign: (draft: AlbumDesignProjectDraft) =>
      commitProjectSettingsDraft(draft),
    applyWithOutcome,
    applyPhotoWithStatus: applyWithOutcome,
    importPhoto: async () => {
      let selectedMediaId: string | null = null;
      const completed = await runWithErrorFeedback(
        async (port) => {
          const result = await port.importPhoto();
          if (result.kind !== "cancelled") selectedMediaId = result.mediaId;
          return result.projection;
        },
      );
      return completed ? selectedMediaId : null;
    },
    dropPhoto: applyWithOutcome,
    applyDpi: async (dpi: number) => {
      await commitInteraction({
        kind: "setDpi",
        dpi,
      });
    },
    relinkMedia: (mediaId: string) =>
      void runWithErrorFeedback((port) =>
        port.relink(mediaId),
      ),
    save: () => void saveVisibleRevision(),
    saveAs: () => void saveVisibleRevisionAs(),
    undo: () => void runHistoryCommand("canUndo", "undo"),
    redo: () => void runHistoryCommand("canRedo", "redo"),
    dismissFeedback: () => {
      setMessage(null);
    },
  };
}
