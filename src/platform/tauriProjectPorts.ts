import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import {
  MediaPreviewError,
  SaveProjectError,
  type ExportPort,
  type ExportProgressEvent,
  type MediaPreviewPort,
  type ProjectSessionPort,
  type SaveProjectFailureCode,
  type SaveProjectOutcome as ApplicationSaveProjectOutcome,
  type SaveProjectResult as ApplicationSaveProjectResult,
} from "../application/projectPorts";
import type { CancelDisposition as IpcCancelDisposition } from "./generated/CancelDisposition";
import type { ExportCommandError as IpcExportCommandError } from "./generated/ExportCommandError";
import type { ExportEvent as IpcExportEvent } from "./generated/ExportEvent";
import type { ExportResult as IpcExportResult } from "./generated/ExportResult";
import type { MediaPreview as IpcMediaPreview } from "./generated/MediaPreview";
import type { MediaPreviewCommandError as IpcMediaPreviewCommandError } from "./generated/MediaPreviewCommandError";
import type { SaveProjectCommandError as IpcSaveProjectCommandError } from "./generated/SaveProjectCommandError";
import type { SaveProjectOutcome as IpcSaveProjectOutcome } from "./generated/SaveProjectOutcome";
import type { SaveProjectResult as IpcSaveProjectResult } from "./generated/SaveProjectResult";

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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function isRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

const saveProjectErrorMapping: Readonly<
  Record<
    IpcSaveProjectCommandError["code"],
    { code: SaveProjectFailureCode; message: string }
  >
> = {
  stale_revision: {
    code: "stale_revision",
    message:
      "A revisão visível ficou desatualizada. Atualize o Projeto e tente salvar novamente.",
  },
  persisted_baseline_conflict: {
    code: "persisted_baseline_conflict",
    message:
      "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
  },
  save_state_indeterminate: {
    code: "save_state_indeterminate",
    message:
      "Não foi possível confirmar qual revisão ficou no arquivo. Reabra o Projeto antes de continuar.",
  },
  session_unavailable: {
    code: "session_unavailable",
    message:
      "A Sessão do Projeto não está mais disponível. Reabra o Projeto para continuar.",
  },
  not_found: {
    code: "not_found",
    message:
      "O arquivo do Projeto não foi encontrado. Confirme se ele foi movido ou removido.",
  },
  unavailable: {
    code: "unavailable",
    message:
      "O local do Projeto está indisponível. Reconecte a unidade ou o compartilhamento e tente novamente.",
  },
  access_denied: {
    code: "access_denied",
    message:
      "O Windows negou acesso ao arquivo do Projeto. Verifique as permissões e tente novamente.",
  },
  invalid_path: {
    code: "invalid_path",
    message: "O caminho do arquivo do Projeto não é válido.",
  },
  unexpected_object_type: {
    code: "unexpected_object_type",
    message: "O destino do Projeto deixou de ser um arquivo regular.",
  },
  conflict: {
    code: "conflict",
    message:
      "O arquivo do Projeto mudou durante o Salvamento. Tente novamente.",
  },
  io_failure: {
    code: "io_failure",
    message: "O Windows não conseguiu concluir o Salvamento do Projeto.",
  },
};

function parseIpcSaveProjectCommandError(
  error: unknown,
): IpcSaveProjectCommandError | null {
  if (!isRecord(error) || typeof error.code !== "string") {
    return null;
  }

  switch (error.code) {
    case "stale_revision":
      return isRevision(error.expectedRevision) &&
        isRevision(error.currentRevision)
        ? {
            code: error.code,
            expectedRevision: error.expectedRevision,
            currentRevision: error.currentRevision,
          }
        : null;
    case "persisted_baseline_conflict":
    case "save_state_indeterminate":
    case "session_unavailable":
    case "not_found":
    case "unavailable":
    case "access_denied":
    case "invalid_path":
    case "unexpected_object_type":
    case "conflict":
    case "io_failure":
      return { code: error.code };
    default:
      return null;
  }
}

function toSaveProjectError(error: unknown): SaveProjectError {
  const ipcError = parseIpcSaveProjectCommandError(error);
  if (!ipcError) {
    return new SaveProjectError(
      "save_unavailable",
      "Não foi possível iniciar o Salvamento do Projeto.",
    );
  }

  const mapping = saveProjectErrorMapping[ipcError.code];

  if (ipcError.code === "stale_revision") {
    return new SaveProjectError(mapping.code, mapping.message, {
      expected: ipcError.expectedRevision,
      current: ipcError.currentRevision,
    });
  }

  return new SaveProjectError(mapping.code, mapping.message);
}

function invalidSaveResponse() {
  return new SaveProjectError(
    "invalid_response",
    "Não foi possível confirmar o resultado do Salvamento.",
  );
}

function parseIpcSaveProjectResult(value: unknown): IpcSaveProjectResult {
  if (!isRecord(value) || !isRecord(value.outcome)) {
    throw invalidSaveResponse();
  }

  const { outcome, projection } = value;
  if (
    (outcome.kind !== "saved" &&
      outcome.kind !== "alreadyCurrent") ||
    !isRevision(outcome.revision) ||
    !isRecord(projection) ||
    !isRecord(projection.state) ||
    typeof projection.state.projectId !== "string" ||
    !isRevision(projection.state.revision) ||
    !isRevision(projection.state.savedRevision) ||
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

export const tauriProjectSessionPort: ProjectSessionPort = {
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

export const tauriMediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: () =>
    invoke<IpcMediaPreview[] | null>("prepare_media_previews").catch(
      (error: unknown) => {
        throw normalizeMediaPreviewError(error);
      },
    ),
};

export const tauriExportPort: ExportPort = {
  startPreview: (emitEvent: (event: ExportProgressEvent) => void) => {
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
    const completion = invoke<IpcExportResult>("export_preview", { onEvent })
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
