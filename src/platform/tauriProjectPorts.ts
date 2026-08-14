import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import {
  MediaPreviewError,
  SaveProjectError,
  type ExportPipelinePort,
  type ExportProgressEvent,
  type MediaPreviewPort,
  type ProjectStartupPort,
  type ProjectCorePort,
  type SaveProjectOutcome as ApplicationSaveProjectOutcome,
  type SaveProjectResult as ApplicationSaveProjectResult,
} from "../application/projectPorts";
import type { CancelDisposition as IpcCancelDisposition } from "./generated/CancelDisposition";
import type { ExportCommandError as IpcExportCommandError } from "./generated/ExportCommandError";
import type { ExportEvent as IpcExportEvent } from "./generated/ExportEvent";
import type { ExportResult as IpcExportResult } from "./generated/ExportResult";
import type { LinkedMediaChanged as IpcLinkedMediaChanged } from "./generated/LinkedMediaChanged";
import type { MediaPreview as IpcMediaPreview } from "./generated/MediaPreview";
import type { MediaPreviewCommandError as IpcMediaPreviewCommandError } from "./generated/MediaPreviewCommandError";
import type { SaveProjectOutcome as IpcSaveProjectOutcome } from "./generated/SaveProjectOutcome";
import type { SaveProjectResult as IpcSaveProjectResult } from "./generated/SaveProjectResult";
import { isIpcRecord, isIpcRevision } from "./ipcGuards";
import { parseProjectSaveFailure } from "./projectSaveFailure";

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

function invalidSaveResponse() {
  return new SaveProjectError(
    "invalid_response",
    "Não foi possível confirmar o resultado do Salvamento.",
  );
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
    !isIpcRecord(projection) ||
    !isIpcRecord(projection.state) ||
    typeof projection.state.projectId !== "string" ||
    !isIpcRevision(projection.state.revision) ||
    !isIpcRevision(projection.state.savedRevision) ||
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
  apply: (intent: ProjectIntent) =>
    invoke<EditorProjection>("apply_project_intent", { intent }),
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
};

export const tauriProjectStartupPort: ProjectStartupPort = {
  confirmUiReady: () => invoke<void>("project_ui_ready"),
};

export const tauriMediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: (demand) =>
    invoke<IpcMediaPreview[] | null>("prepare_media_previews", { demand }).catch(
      (error: unknown) => {
        throw normalizeMediaPreviewError(error);
      },
    ),
  onMediaChanged: (listener) =>
    listen<IpcLinkedMediaChanged>(
      "myalbuns://linked-media-changed",
      ({ payload }) => listener(payload.mediaIds),
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
