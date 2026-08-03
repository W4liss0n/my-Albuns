import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import type {
  ExportPort,
  ExportProgressEvent,
  MediaPreviewPort,
  ProjectSessionPort,
} from "../application/projectPorts";
import type { CancelDisposition as IpcCancelDisposition } from "./generated/CancelDisposition";
import type { ExportCommandError as IpcExportCommandError } from "./generated/ExportCommandError";
import type { ExportEvent as IpcExportEvent } from "./generated/ExportEvent";
import type { ExportResult as IpcExportResult } from "./generated/ExportResult";
import type { MediaPreview as IpcMediaPreview } from "./generated/MediaPreview";

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

export const tauriProjectSessionPort: ProjectSessionPort = {
  load: (operationId) =>
    invoke<EditorProjection>("project_state", { operationId }),
  apply: (intent: ProjectIntent) =>
    invoke<EditorProjection>("apply_project_intent", { intent }),
  undo: () => invoke<EditorProjection>("undo_project"),
  redo: () => invoke<EditorProjection>("redo_project"),
};

export const tauriMediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: () =>
    invoke<IpcMediaPreview[] | null>("prepare_media_previews"),
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
