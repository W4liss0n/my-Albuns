import { Channel, invoke } from "@tauri-apps/api/core";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import type {
  ExportCancelStatus,
  ExportPort,
  ExportProgressEvent,
  ExportProgressStage,
  ExportProgressUnits,
  ExportResult,
  MediaPreview,
  MediaPreviewPort,
  ProjectSessionPort,
} from "../application/projectPorts";

type TauriExportEvent =
  | {
      event: "started";
      data: {
        operationId: string;
        cancellable: boolean;
      };
    }
  | {
      event: "progress";
      data: {
        operationId: string;
        stage: ExportProgressStage;
        units: ExportProgressUnits;
        cancellable: boolean;
      };
    };

function isCancelledExportError(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    error.code === "cancelled"
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
    invoke<MediaPreview[] | null>("prepare_media_previews"),
};

export const tauriExportPort: ExportPort = {
  startPreview: (emitEvent: (event: ExportProgressEvent) => void) => {
    const onEvent = new Channel<TauriExportEvent>();
    let correlationSettled = false;
    let resolveCorrelation: (operationId: string | null) => void = () =>
      undefined;
    const correlation = new Promise<string | null>((resolve) => {
      resolveCorrelation = resolve;
    });
    let cancellation: Promise<ExportCancelStatus> | undefined;
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
    const completion = invoke<ExportResult>("export_spike", { onEvent })
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
            : invoke<ExportCancelStatus>("cancel_export_spike", {
                operationId,
              }),
        );
        return cancellation;
      },
    };
  },
};
