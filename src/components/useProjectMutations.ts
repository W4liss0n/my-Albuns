import { useEffect, useRef, useState } from "react";
import type { PhotoImportCompletion, ImageProcessingProgress } from "../application/projectPorts";
import { createLogInstanceId } from "../application/logging";
import { useImageProcessing } from "./useImageProcessing";
import type { CanvasPhotoDropPoint } from "./albumCanvasContract";
import type { PrepareImportedMedia } from "../application/mediaPreviews";

import type {
  ComposedFrame,
  EditorProjection,
  FrameGeometryEdit,
  PhotoOrientationAction,
  PhotoAngleEdit,
  FrameStyleEdit,
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
  const [pendingFrameCopies, setPendingFrameCopies] = useState(0);
  const frameCopySessionRef = useRef({ pending: 0 });

  useEffect(() => {
    setMessage(null);
    importAttemptRef.current = { pending: false };
    setImportPending(false);
    setPhotoImportResult(null);
    frameCopySessionRef.current = { pending: 0 };
    setPendingFrameCopies(0);
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
    onCompleted?: (next: EditorProjection) => void,
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
      onCompleted?.(outcome.projection);
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

  async function swapSheetSides(sheetId: string) {
    let applied = false;
    const completed = await runWithErrorFeedback(async (port, latestProjection) => {
      const current = latestProjection ?? projection;
      const target = current.state.album.sheets.find((sheet) => sheet.id === sheetId);
      if (target?.activeSides !== "both") return current;
      const next = await imageProcessing.run((publish) => port.apply({ kind: "swapSheetSides", sheetId }, publish));
      applied = true;
      return next;
    }, true);
    return completed && applied;
  }

  function orientPhotos(frameIds: string[], action: PhotoOrientationAction) {
    return runWithErrorFeedback(
      (port) => imageProcessing.run((publish) =>
        port.apply({ kind: "orientPhotos", frameIds, action }, publish)),
      true,
    );
  }

  function togglePhotoBlackAndWhite(frameIds: string[]) {
    return runWithErrorFeedback(
      (port) => imageProcessing.run((publish) =>
        port.apply({ kind: "togglePhotoBlackAndWhite", frameIds }, publish)),
      true,
    );
  }

  async function commitPhotoAngle(edit: PhotoAngleEdit): Promise<EditorProjection | null> {
    let committed: EditorProjection | null = null;
    await runWithErrorFeedback(
      (port) => imageProcessing.run((publish) => port.apply({ kind: "setPhotoAngle", edit }, publish)),
      true,
      (next) => { committed = next; },
    );
    return committed;
  }

  async function commitFrameStyle(edit: FrameStyleEdit): Promise<EditorProjection | null> {
    let committed: EditorProjection | null = null;
    await runWithErrorFeedback(
      (port) => imageProcessing.run((publish) => port.apply({ kind: "setFrameStyle", edit }, publish)),
      true,
      (next) => { committed = next; },
    );
    return committed;
  }

  async function copyFrames(frameIds: string[]) {
    const session = frameCopySessionRef.current;
    session.pending += 1;
    setPendingFrameCopies(session.pending);
    try {
      return await runWithErrorFeedback((port) => port.apply({ kind: "copyFrames", frameIds }), true);
    } finally {
      session.pending -= 1;
      if (frameCopySessionRef.current === session) setPendingFrameCopies(session.pending);
    }
  }

  function pasteFrames(sheetId: string, desiredOffsetUm: number, selectPasted: (ids: string[], next: EditorProjection) => void) {
    let ids: string[] = [];
    return runWithErrorFeedback(async (port, latestProjection) => {
      const current = latestProjection ?? projection;
      if (!current.canPasteFrames) return current;
      const result = await imageProcessing.run((publish) => port.applyWithOutcome({ kind: "pasteFrames", sheetId, desiredOffsetUm }, publish));
      ids = result.affectedFrameIds ?? [];
      return result.projection;
    }, true, (next) => { if (ids.length > 0) selectPasted(ids, next); });
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
    return (await commitProjection((port, latestProjection) =>
      imageProcessing.run((publish) => port.apply(
        materializeProjectIntent(
          intent,
          capturedProjection,
          latestProjection ?? capturedProjection,
        ),
        publish,
      )),
    )) !== null;
  }

  async function commitFrameGeometry(edit: FrameGeometryEdit): Promise<ComposedFrame[] | null> {
    const committed = await commitProjection((port) =>
      imageProcessing.run((publish) => port.apply({ kind: "editFrameGeometry", edit }, publish)),
    );
    return committed?.composition.sheets
      .flatMap((sheet) => sheet.frames)
      .filter((frame) => edit.frames.some((target) => target.frameId === frame.frameId)) ?? null;
  }

  async function swapFrameContentsAtPoint(sourceFrameId: string, point: CanvasPhotoDropPoint) {
    let applied = false;
    const expectedPhoto = projection.state.album.sheets.flatMap((sheet) => sheet.frames)
      .find((frame) => frame.id === sourceFrameId)?.photo;
    // Include hit testing in the queue: a following Save/Undo must not overtake the drop.
    const completed = await runWithErrorFeedback(async (port, latestProjection) => {
      const current = latestProjection ?? projection;
      const source = current.state.album.sheets.flatMap((sheet) => sheet.frames)
        .find((frame) => frame.id === sourceFrameId);
      if (!source?.photo || JSON.stringify(source.photo) !== JSON.stringify(expectedPhoto) ||
          !current.state.album.sheets.some((sheet) => sheet.id === point.sheetId)) return current;
      const target = await port.resolvePhotoDropTarget(point.sheetId, point.xUm, point.yUm);
      if (target.kind !== "frame" || target.frameId === sourceFrameId) return current;
      const swapped = await imageProcessing.run((publish) => port.apply({
        kind: "swapFrameContents", frameIds: [sourceFrameId, target.frameId],
      }, publish));
      applied = true;
      return swapped;
    }, true);
    return completed && applied;
  }

  async function commitProjectSettingsDraft<Value, Delta>(
    draft: ProjectSettingsDraft<Value, Delta>,
  ) {
    return (await commitProjection((port, latestProjection) => {
      const effectiveProjection = latestProjection ?? projection;
      const materialized = draft.materializeAgainst(effectiveProjection);
      return materialized.changed
        ? imageProcessing.run((publish) => port.apply(materialized.intent, publish))
        : Promise.resolve(effectiveProjection);
    })) !== null;
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

  async function commitProjection(operation: ProjectMutationOperation) {
    if (saveAsBarrierRef.current) return null;
    setMessage(null);
    const outcome = await runProjectMutation.run(operation);
    if (outcome.status === "completed") {
      onProjectionChange(outcome.projection);
      return outcome.projection;
    }
    if (outcome.status === "failed") {
      setMessage(messageFromError(outcome.error));
    }
    return null;
  }

  return {
    reportInteractionError: setMessage,
    message,
    importPending,
    imageProcessingProgress: imageProcessing.progress,
    imageProcessingProblems: imageProcessing.problems,
    dismissImageProcessingProblems: imageProcessing.dismissProblems,
    photoImportResult,
    applyIntent,
    commitInteraction,
    commitFrameGeometry,
    swapFrameContentsAtPoint,
    swapSheetSides,
    orientPhotos,
    togglePhotoBlackAndWhite,
    commitPhotoAngle,
    commitFrameStyle,
    applyAlbumInformation: commitAlbumInformation,
    applyAlbumDesign: (draft: AlbumDesignProjectDraft) =>
      commitProjectSettingsDraft(draft),
    applyWithOutcome,
    applyPhotoWithStatus: applyWithOutcome,
    copyFrames,
    pasteFrames,
    frameCopyPending: pendingFrameCopies > 0,
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
