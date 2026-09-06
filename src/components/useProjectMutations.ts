import { useEffect, useRef, useState } from "react";
import type { PhotoImportCompletion, ImageProcessingProgress } from "../application/projectPorts";
import { createLogInstanceId } from "../application/logging";
import { useImageProcessing } from "./useImageProcessing";
import type { PrepareImportedMedia } from "../application/mediaPreviews";

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
import {
  isSheetStructureIntent,
  materializeSheetStructureIntent,
} from "../application/sheetStructure";
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
  prepareImportedMedia?: PrepareImportedMedia;
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
  prepareImportedMedia,
}: ProjectMutationsInput) {
  const [message, setMessage] = useState<string | null>(null);
  const [importPending, setImportPending] = useState(false);
  const imageProcessing = useImageProcessing(projection.state.projectId, runProjectMutation);
  const importAttemptRef = useRef({ pending: false });
  const [photoImportResult, setPhotoImportResult] = useState<PhotoImportCompletion | null>(null);
  const feedbackTokenRef = useRef(0);
  const saveAsBarrierRef = useRef(false);

  useEffect(() => {
    setMessage(null);
    importAttemptRef.current = { pending: false };
    setImportPending(false);
    setPhotoImportResult(null);
    return () => { importAttemptRef.current = { pending: false }; };
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
      imageProcessing.run((publish) => port.apply(
        materializeProjectIntent(
          intent,
          capturedProjection,
          latestProjection ?? capturedProjection,
        ),
        publish,
      )),
    );
  }

  async function applyWithOutcome(intent: ProjectIntent) {
    const capturedProjection = projection;
    let affectedFrameId: string | null = null;
    let affectedSheetId: string | null = null;
    let structuralIntentCancelled = false;
    const completed = await runWithErrorFeedback(
      async (port, latestProjection) => {
        const effectiveProjection = latestProjection ?? capturedProjection;
        let materializedIntent = intent;
        if (
          isSheetStructureIntent(intent) &&
          (latestProjection !== null || intent.kind === "reorderSheet")
        ) {
          const materializedStructure = materializeSheetStructureIntent(
            capturedProjection.state.album.sheets,
            effectiveProjection.state.album.sheets,
            intent,
          );
          if (materializedStructure === null) {
            structuralIntentCancelled = true;
            return effectiveProjection;
          }
          materializedIntent = materializedStructure;
        }
        const result = await imageProcessing.run((publish) => port.applyWithOutcome(materializedIntent, publish));
        affectedFrameId = result.affectedFrameId;
        affectedSheetId = result.affectedSheetId;
        return result.projection;
      },
    );
    if (structuralIntentCancelled) return false;
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
        return imageProcessing.run((publish) => port[operation](publish));
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
      imageProcessing.run((publish) => port.apply(
        materializeProjectIntent(
          intent,
          capturedProjection,
          latestProjection ?? capturedProjection,
        ),
        publish,
      )),
    );
  }

  function commitProjectSettingsDraft<Value, Delta>(
    draft: ProjectSettingsDraft<Value, Delta>,
  ) {
    return commitMutation((port, latestProjection) => {
      const effectiveProjection = latestProjection ?? projection;
      const materialized = draft.materializeAgainst(effectiveProjection);
      return materialized.changed
        ? imageProcessing.run((publish) => port.apply(materialized.intent, publish))
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
        return imageProcessing.run((publish) => port.apply(materialized.intent, publish));
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
    importPending,
    imageProcessingProgress: imageProcessing.progress,
    imageProcessingProblems: imageProcessing.problems,
    dismissImageProcessingProblems: imageProcessing.dismissProblems,
    photoImportResult,
    applyIntent,
    commitInteraction,
    applyAlbumInformation: commitAlbumInformation,
    applyAlbumDesign: (draft: AlbumDesignProjectDraft) =>
      commitProjectSettingsDraft(draft),
    applyWithOutcome,
    applyPhotoWithStatus: applyWithOutcome,
    importPhoto: async () => {
      if (importAttemptRef.current.pending || saveAsBarrierRef.current) return null;
      const attempt = { pending: true };
      importAttemptRef.current = attempt;
      setImportPending(true);
      setPhotoImportResult(null);
      let result: PhotoImportCompletion | null = null;
      try {
        const completed = await runWithErrorFeedback(async (port) => {
          const imported = await imageProcessing.run(async (publish) => {
            const imported = await port.importPhoto(publish);
            if (imported.kind !== "completed" || imported.mediaIds.length === 0 || !prepareImportedMedia) return imported;
            const problems = await prepareImportedMedia(imported);
            return { ...imported, problems: [...imported.problems, ...problems] };
          });
          if (imported.kind === "completed") result = imported;
          return imported.projection;
        });
        if (!completed || importAttemptRef.current !== attempt) return null;
        const completion = result as PhotoImportCompletion | null;
        setPhotoImportResult(completion);
        return completion?.mediaIds[completion.mediaIds.length - 1] ?? null;
      } finally {
        if (importAttemptRef.current === attempt) {
          attempt.pending = false;
          setImportPending(false);
        }
      }
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
        imageProcessing.run((publish) => port.relink(mediaId, publish)),
      ),
    retryUnavailableMedia: async (retry: (publish: (progress: ImageProcessingProgress) => void) => Promise<void>) => {
      await runWithErrorFeedback(async (port) => {
        await imageProcessing.run(retry);
        return port.load(createLogInstanceId("media-retry"));
      });
    },
    save: () => void saveVisibleRevision(),
    saveAs: () => void saveVisibleRevisionAs(),
    undo: () => void runHistoryCommand("canUndo", "undo"),
    redo: () => void runHistoryCommand("canRedo", "redo"),
    dismissFeedback: () => setMessage(null),
    dismissPhotoImportResult: () => setPhotoImportResult(null),
  };
}
