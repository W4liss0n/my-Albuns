import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  EditorProjection,
  PhotoDropTarget,
  ProjectIntent,
  ProjectMutationOutcome,
} from "../domain/project";
import {
  MediaPreviewError,
  SaveProjectError,
  type ExportPipelinePort,
  type ExportProgressEvent,
  type MediaPreviewPort,
  type ProjectStartupPort,
  type ProjectCorePort,
  type SaveAsProjectOutcome as ApplicationSaveAsProjectOutcome,
  type SaveAsProjectResult as ApplicationSaveAsProjectResult,
  type SaveProjectOutcome as ApplicationSaveProjectOutcome,
  type SaveProjectResult as ApplicationSaveProjectResult,
} from "../application/projectPorts";
import type { CancelDisposition as IpcCancelDisposition } from "./generated/CancelDisposition";
import type { CacheProcessorWarning as IpcCacheProcessorWarning } from "./generated/CacheProcessorWarning";
import type { ExportCommandError as IpcExportCommandError } from "./generated/ExportCommandError";
import type { ExportEvent as IpcExportEvent } from "./generated/ExportEvent";
import type { ExportResult as IpcExportResult } from "./generated/ExportResult";
import type { ImportPhotoResult as IpcImportPhotoResult } from "./generated/ImportPhotoResult";
import type { LinkedMediaChanged as IpcLinkedMediaChanged } from "./generated/LinkedMediaChanged";
import type { MediaPreview as IpcMediaPreview } from "./generated/MediaPreview";
import type { MediaPreviewCommandError as IpcMediaPreviewCommandError } from "./generated/MediaPreviewCommandError";
import type { ProjectRecoveryChoice as IpcProjectRecoveryChoice } from "./generated/ProjectRecoveryChoice";
import type { ProjectRecoveryResolution as IpcProjectRecoveryResolution } from "./generated/ProjectRecoveryResolution";
import type { ProjectRecoveryStatus as IpcProjectRecoveryStatus } from "./generated/ProjectRecoveryStatus";
import type { SaveProjectOutcome as IpcSaveProjectOutcome } from "./generated/SaveProjectOutcome";
import type { SaveProjectResult as IpcSaveProjectResult } from "./generated/SaveProjectResult";
import type { SaveAsProjectOutcome as IpcSaveAsProjectOutcome } from "./generated/SaveAsProjectOutcome";
import type { SaveAsProjectResult as IpcSaveAsProjectResult } from "./generated/SaveAsProjectResult";
import {
  hasOnlyIpcKeys,
  isIpcEditorProjection,
  isIpcRecord,
  isIpcRevision,
} from "./ipcGuards";
import { parseProjectSaveFailure } from "./projectSaveFailure";
import { parseProjectSaveAsFailure } from "./projectSaveAsFailure";

function isMediaPreviewCommandError(
  error: unknown,
): error is IpcMediaPreviewCommandError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error.code === "unavailable" ||
      error.code === "unsupported_image" ||
      error.code === "read_failed") &&
    "message" in error &&
    typeof error.message === "string"
  );
}

function normalizeMediaPreviewError(error: unknown) {
  if (isMediaPreviewCommandError(error)) {
    return new MediaPreviewError(error.code, error.message);
  }
  if (error instanceof Error) {
    return new MediaPreviewError("read_failed", error.message);
  }
  return new MediaPreviewError(
    "read_failed",
    "Não foi possível preparar as Prévias de mídia vinculada.",
  );
}

function isCancelledExportError(
  error: unknown,
): error is IpcExportCommandError {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "cancelled" &&
    "message" in error &&
    typeof error.message === "string"
  );
}

function toSaveProjectError(error: unknown): SaveProjectError {
  const failure = parseProjectSaveFailure(error);
  if (!failure) {
    return new SaveProjectError(
      "save_unavailable",
      "Não foi possível iniciar o Salvamento do Projeto.",
    );
  }

  return new SaveProjectError(
    failure.code,
    failure.message,
    failure.context,
  );
}

function parseProjectRecoveryStatus(value: unknown): IpcProjectRecoveryStatus {
  if (
    !isIpcRecord(value) ||
    !hasOnlyIpcKeys(value, ["kind"]) ||
    (value.kind !== "none" && value.kind !== "available")
  ) {
    throw new Error("Não foi possível verificar a Recuperação do Projeto.");
  }
  return { kind: value.kind };
}

function parseProjectRecoveryResolution(
  value: unknown,
): IpcProjectRecoveryResolution {
  if (!isIpcRecord(value)) {
    throw new Error("Não foi possível confirmar a escolha de Recuperação.");
  }
  if (value.kind === "deferred" && hasOnlyIpcKeys(value, ["kind"])) {
    return { kind: "deferred" };
  }
  if (
    (value.kind !== "recovered" && value.kind !== "openedLastSaved") ||
    !hasOnlyIpcKeys(value, ["kind", "projection"]) ||
    !isIpcEditorProjection(value.projection)
  ) {
    throw new Error("Não foi possível confirmar a escolha de Recuperação.");
  }
  return {
    kind: value.kind,
    projection: value.projection,
  };
}

function invalidSaveResponse() {
  return new SaveProjectError(
    "invalid_response",
    "Não foi possível confirmar o resultado do Salvamento.",
  );
}

function toSaveAsProjectError(error: unknown): SaveProjectError {
  const failure = parseProjectSaveAsFailure(error);
  if (!failure) {
    return new SaveProjectError(
      "save_unavailable",
      "Não foi possível iniciar Salvar como.",
    );
  }
  return new SaveProjectError(failure.code, failure.message, failure.context);
}

function invalidSaveAsResponse() {
  return new SaveProjectError(
    "invalid_response",
    "Não foi possível confirmar o resultado de Salvar como.",
  );
}

function isProjectionIdentity(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function isCanonicalProjectIdentity(value: unknown): value is string {
  return (
    typeof value === "string" &&
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(
      value,
    )
  );
}

function parseIpcSaveAsProjectResult(
  value: unknown,
  expectedRevision: number,
): IpcSaveAsProjectResult {
  if (!isIpcRecord(value) || !isIpcRecord(value.outcome)) {
    throw invalidSaveAsResponse();
  }

  const { outcome, projection } = value;
  if (
    !isIpcEditorProjection(projection)
  ) {
    throw invalidSaveAsResponse();
  }

  if (outcome.kind === "cancelled") {
    if (projection.state.revision !== expectedRevision) {
      throw invalidSaveAsResponse();
    }
    return {
      outcome: { kind: "cancelled" },
      projection: projection as IpcSaveAsProjectResult["projection"],
    };
  }
  if (
    outcome.kind !== "savedAs" ||
    !isProjectionIdentity(outcome.previousProjectId) ||
    !isCanonicalProjectIdentity(outcome.projectId) ||
    outcome.previousProjectId === outcome.projectId ||
    !isIpcRevision(outcome.revision) ||
    outcome.revision !== expectedRevision ||
    projection.state.projectId !== outcome.projectId ||
    projection.state.revision !== outcome.revision ||
    projection.state.savedRevision !== outcome.revision ||
    projection.state.dirty !== false ||
    typeof projection.state.projectName !== "string" ||
    projection.state.projectName.length === 0
  ) {
    throw invalidSaveAsResponse();
  }

  return {
    outcome: {
      kind: "savedAs",
      previousProjectId: outcome.previousProjectId,
      projectId: outcome.projectId,
      revision: outcome.revision,
    },
    projection: projection as IpcSaveAsProjectResult["projection"],
  };
}

function toApplicationSaveAsProjectOutcome(
  outcome: IpcSaveAsProjectOutcome,
): ApplicationSaveAsProjectOutcome {
  return outcome.kind === "cancelled"
    ? { kind: "cancelled" }
    : {
        kind: "savedAs",
        previousProjectId: outcome.previousProjectId,
        projectId: outcome.projectId,
        revision: outcome.revision,
      };
}

function toSaveAsProjectResult(
  value: unknown,
  expectedRevision: number,
): ApplicationSaveAsProjectResult {
  const ipcResult = parseIpcSaveAsProjectResult(value, expectedRevision);
  return {
    outcome: toApplicationSaveAsProjectOutcome(ipcResult.outcome),
    projection: ipcResult.projection,
  };
}

function parseIpcSaveProjectResult(value: unknown): IpcSaveProjectResult {
  if (!isIpcRecord(value) || !isIpcRecord(value.outcome)) {
    throw invalidSaveResponse();
  }

  const { outcome, projection } = value;
  if (
    (outcome.kind !== "saved" &&
      outcome.kind !== "alreadyCurrent") ||
    !isIpcRevision(outcome.revision) ||
    !isIpcEditorProjection(projection) ||
    projection.state.revision !== outcome.revision ||
    projection.state.savedRevision !== outcome.revision
  ) {
    throw invalidSaveResponse();
  }

  const ipcOutcome: IpcSaveProjectOutcome = {
    kind: outcome.kind,
    revision: outcome.revision,
  };

  return {
    outcome: ipcOutcome,
    projection: projection as IpcSaveProjectResult["projection"],
  };
}

function toApplicationSaveProjectOutcome(
  outcome: IpcSaveProjectOutcome,
): ApplicationSaveProjectOutcome {
  switch (outcome.kind) {
    case "saved":
      return { kind: "saved", revision: outcome.revision };
    case "alreadyCurrent":
      return { kind: "alreadyCurrent", revision: outcome.revision };
  }
}

function toSaveProjectResult(value: unknown): ApplicationSaveProjectResult {
  const ipcResult = parseIpcSaveProjectResult(value);
  return {
    outcome: toApplicationSaveProjectOutcome(ipcResult.outcome),
    projection: ipcResult.projection,
  };
}

export const tauriProjectCorePort: ProjectCorePort = {
  load: (operationId) =>
    invoke<EditorProjection>("project_state", { operationId }),
  apply: async (intent: ProjectIntent) =>
    (
      await invoke<ProjectMutationOutcome>("apply_project_intent", {
        intent,
      })
    ).projection,
  applyWithOutcome: (intent: ProjectIntent) =>
    invoke<ProjectMutationOutcome>("apply_project_intent", { intent }),
  importPhoto: () => invoke<IpcImportPhotoResult>("import_photo"),
  resolvePhotoDropTarget: (
    sheetId: string,
    xUm: number,
    yUm: number,
  ) =>
    invoke<PhotoDropTarget>("photo_drop_target", {
      sheetId,
      xUm,
      yUm,
    }),
  relink: (mediaId) =>
    invoke<EditorProjection>("relink_media", { mediaId }),
  undo: () => invoke<EditorProjection>("undo_project"),
  redo: () => invoke<EditorProjection>("redo_project"),
  save: async (expectedRevision) => {
    try {
      return toSaveProjectResult(
        await invoke<unknown>("save_project", { expectedRevision }),
      );
    } catch (error: unknown) {
      throw error instanceof SaveProjectError
        ? error
        : toSaveProjectError(error);
    }
  },
  saveAs: async (expectedRevision) => {
    try {
      return toSaveAsProjectResult(
        await invoke<unknown>("save_project_as", { expectedRevision }),
        expectedRevision,
      );
    } catch (error: unknown) {
      throw error instanceof SaveProjectError
        ? error
        : toSaveAsProjectError(error);
    }
  },
};

export const tauriProjectStartupPort: ProjectStartupPort = {
  recoveryStatus: async () =>
    parseProjectRecoveryStatus(
      await invoke<unknown>("project_recovery_status"),
    ),
  resolveRecovery: async (choice, checkpointDiscardConfirmed) =>
    parseProjectRecoveryResolution(
      await invoke<unknown>("resolve_project_recovery", {
        choice: choice satisfies IpcProjectRecoveryChoice,
        checkpointDiscardConfirmed,
      }),
    ),
  confirmUiReady: () => invoke<void>("project_ui_ready"),
};

export const tauriMediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: (demand) =>
    invoke<IpcMediaPreview[] | null>("prepare_media_previews", { demand }).catch(
      (error: unknown) => {
        throw normalizeMediaPreviewError(error);
      },
    ),
  retryUnavailableMedia: (mediaId) =>
    invoke<IpcMediaPreview>("retry_unavailable_media", { mediaId }).catch(
      (error: unknown) => {
        throw normalizeMediaPreviewError(error);
      },
    ),
  onMediaChanged: (listener) =>
    listen<IpcLinkedMediaChanged>(
      "myalbuns://linked-media-changed",
      ({ payload }) => listener(payload.mediaIds),
    ),
  onCacheProcessorWarning: (listener) =>
    listen<IpcCacheProcessorWarning>(
      "myalbuns://cache-processor-warning",
      ({ payload }) => listener(payload),
    ),
};

export const tauriExportPipelinePort: ExportPipelinePort = {
  startSheet: (
    { projectName, sheetId, sheetNumber },
    emitEvent: (event: ExportProgressEvent) => void,
  ) => {
    const onEvent = new Channel<IpcExportEvent>();
    let correlationSettled = false;
    let resolveCorrelation: (operationId: string | null) => void = () =>
      undefined;
    const correlation = new Promise<string | null>((resolve) => {
      resolveCorrelation = resolve;
    });
    let cancellation: Promise<IpcCancelDisposition> | undefined;
    const settleCorrelation = (operationId: string | null) => {
      if (correlationSettled) {
        return;
      }

      correlationSettled = true;
      resolveCorrelation(operationId);
    };

    onEvent.onmessage = (event) => {
      if (event.event === "started") {
        settleCorrelation(event.data.operationId);
        emitEvent({
          event: "started",
          cancellable: event.data.cancellable,
        });
        return;
      }

      emitEvent({
        event: "progress",
        stage: event.data.stage,
        units: event.data.units,
        cancellable: event.data.cancellable,
      });
    };
    const completion = invoke<IpcExportResult>("export_sheet", {
      projectName,
      sheetId,
      sheetNumber,
      onEvent,
    })
      .then((result) => ({
        status: "completed" as const,
        result,
      }))
      .catch((error: unknown) => {
        if (isCancelledExportError(error)) {
          return {
            status: "cancelled" as const,
          };
        }

        throw error;
      })
      .finally(() => {
        settleCorrelation(null);
      });

    return {
      completion,
      cancel: () => {
        cancellation ??= correlation.then((operationId) =>
          operationId === null
            ? "not_found"
            : invoke<IpcCancelDisposition>("cancel_export", {
                operationId,
              }),
        );
        return cancellation;
      },
    };
  },
};
