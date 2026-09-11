import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type {
  AlbumInformation,
  AlbumInformationValidation,
  EditorProjection,
  ComposedFrame,
  LayoutQueryResult,
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
  type ImageProcessingProgress,
  type ImageProcessingProblem,
  type ProjectStartupPort,
  type ProjectCorePort,
  type SaveAsProjectOutcome as ApplicationSaveAsProjectOutcome,
  type SaveAsProjectResult as ApplicationSaveAsProjectResult,
  type SaveProjectOutcome as ApplicationSaveProjectOutcome,
  type SaveProjectResult as ApplicationSaveProjectResult,
} from "../application/projectPorts";
import type {
  WorkspacePreferenceChange,
  WorkspacePreferences,
  WorkspacePreferencesPort,
} from "../application/workspacePreferences";
import { createWorkspacePreferences } from "../application/workspacePreferences";
import type { ApplicationSettings as IpcApplicationSettings } from "./generated/ApplicationSettings";
import type { CancelDisposition as IpcCancelDisposition } from "./generated/CancelDisposition";
import type { CacheProcessorWarning as IpcCacheProcessorWarning } from "./generated/CacheProcessorWarning";
import type { ExportCommandError as IpcExportCommandError } from "./generated/ExportCommandError";
import { LayoutExportBlockedError } from "../application/projectPorts";
import { parseLayoutExportProblems } from "./layoutExportContract";
import type { ExportEvent as IpcExportEvent } from "./generated/ExportEvent";
import type { ExportResult as IpcExportResult } from "./generated/ExportResult";
import type { ImportMediaResult as IpcImportMediaResult } from "./generated/ImportMediaResult";
import type { ImageProcessingProgress as IpcImageProcessingProgress } from "./generated/ImageProcessingProgress";
import type { LinkedMediaChanged as IpcLinkedMediaChanged } from "./generated/LinkedMediaChanged";
import type { MediaPreview as IpcMediaPreview } from "./generated/MediaPreview";
import type { MediaFileCatalog as IpcMediaFileCatalog } from "./generated/MediaFileCatalog";
import type { MediaPreviewCommandError as IpcMediaPreviewCommandError } from "./generated/MediaPreviewCommandError";
import type { PointerDragThreshold } from "./generated/PointerDragThreshold";
import type { SaveProjectOutcome as IpcSaveProjectOutcome } from "./generated/SaveProjectOutcome";
import type { SaveProjectResult as IpcSaveProjectResult } from "./generated/SaveProjectResult";
import type { SaveAsProjectOutcome as IpcSaveAsProjectOutcome } from "./generated/SaveAsProjectOutcome";
import type { SaveAsProjectResult as IpcSaveAsProjectResult } from "./generated/SaveAsProjectResult";
import type { WorkspacePreferenceChange as IpcWorkspacePreferenceChange } from "./generated/WorkspacePreferenceChange";
import type { WorkspacePreferences as IpcWorkspacePreferences } from "./generated/WorkspacePreferences";
import type { SettingsPreferenceChange as IpcSettingsPreferenceChange } from "./generated/SettingsPreferenceChange";
import {
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

async function invokeImageProcessing<T>(
  command: string,
  args: Record<string, unknown>,
  onProgress?: (progress: ImageProcessingProgress) => void,
): Promise<T> {
  const progressChannel = new Channel<IpcImageProcessingProgress>();
  let active = true;
  progressChannel.onmessage = (progress) => { if (active) onProgress?.(progress); };
  try {
    return await invoke<T>(command, { ...args, onProgress: progressChannel });
  } finally {
    active = false;
  }
}

export const tauriProjectCorePort: ProjectCorePort = {
  readFrameDragThreshold: () => invoke<PointerDragThreshold>("frame_drag_threshold"),
  readSliderDoubleClickTime: () => invoke<number>("slider_double_click_time"),
  previewPhotoAngle: (edit) => invoke<ComposedFrame[]>("preview_photo_angle", { edit }),
  previewDecorativeDrop: (request) => invoke<import("../domain/project").DecorativeDropPreview | null>("preview_decorative_drop", { request }),
  previewFrameStyle: (edit) => invoke<ComposedFrame[]>("preview_frame_style", { edit }),
  queryLayouts: (sheetId, frameRequest) => invoke<LayoutQueryResult>("query_layouts", frameRequest ? { sheetId, frameRequest } : { sheetId }),
  refreshLayoutCatalog: () => invoke<number>("refresh_layout_catalog"),
  saveCustomLayout: (sheetId) => invoke("save_custom_layout", { sheetId }),
  deleteCustomLayout: (layoutId) => invoke<number>("delete_custom_layout", { layoutId }),
  previewLayout: (selection) => invoke<ComposedFrame[]>("preview_layout", { selection }),
  previewFrameGeometry: (edit) => invoke<import("../domain/project").FrameGeometryPreview>("preview_frame_geometry", { edit }),
  load: (operationId) =>
    invoke<EditorProjection>("project_state", { operationId }),
  validateAlbumInformation: (information: AlbumInformation) =>
    invoke<AlbumInformationValidation>("validate_album_information", {
      information,
    }),
  apply: async (intent: ProjectIntent, onProgress) =>
    (
      await invokeImageProcessing<ProjectMutationOutcome>("apply_project_intent", {
        intent,
      }, onProgress)
    ).projection,
  applyWithOutcome: (intent: ProjectIntent, onProgress) =>
    invokeImageProcessing<ProjectMutationOutcome>("apply_project_intent", { intent }, onProgress),
  importMedia: (onProgress, selection) => invokeImageProcessing<IpcImportMediaResult>("import_media", { selection }, onProgress),
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
  relink: (mediaId, onProgress) =>
    invokeImageProcessing<EditorProjection>("relink_media", { mediaId }, onProgress),
  undo: (onProgress) => invokeImageProcessing<EditorProjection>("undo_project", {}, onProgress),
  redo: (onProgress) => invokeImageProcessing<EditorProjection>("redo_project", {}, onProgress),
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
  confirmUiReady: () => invoke<readonly ImageProcessingProblem[]>("project_ui_ready"),
};

async function loadWorkspacePreferences(): Promise<WorkspacePreferences> {
  const [state, settings] = await Promise.all([
    invoke<IpcWorkspacePreferences>("workspace_preferences"),
    invoke<IpcApplicationSettings>("application_settings"),
  ]);
  return createWorkspacePreferences({
    inspectorSections: state.inspectorSections,
    mediaPanel: settings.mediaPanel,
    mediaPanelActiveKind: settings.mediaPanel.activeKind,
    mediaThumbnailSize: state.mediaThumbnailSize,
    workspacePanels: state.workspacePanels,
  });
}

export const tauriWorkspacePreferencesPort: WorkspacePreferencesPort = {
  load: loadWorkspacePreferences,
  update: async (change: WorkspacePreferenceChange) => {
    if (
      change.kind === "mediaPanelSortDirection" ||
      change.kind === "mediaPanelActiveKind" ||
      change.kind === "mediaPanelSortKey" ||
      change.kind === "mediaPanelUsageFilter"
    ) {
      await invoke<IpcApplicationSettings>("update_application_setting", {
        change: change satisfies IpcSettingsPreferenceChange,
      });
    } else {
      await invoke<IpcWorkspacePreferences>("update_workspace_preference", {
        change: change satisfies IpcWorkspacePreferenceChange,
      });
    }
    return loadWorkspacePreferences();
  },
};

export const tauriMediaPreviewPort: MediaPreviewPort = {
  readMediaFiles: () => invoke<IpcMediaFileCatalog>("read_media_files"),
  prepareMediaPreviews: async (demand, publish) => {
    const onPreview = new Channel<IpcMediaPreview>();
    let active = true;
    onPreview.onmessage = (preview) => {
      if (active) publish(preview);
    };
    try {
      return await invoke<IpcMediaPreview[] | null>("prepare_media_previews", {
        demand: {
          revision: demand.revision,
          visibleMediaIds: [...demand.visibleMediaIds],
          preloadMediaIds: [...demand.preloadMediaIds],
        },
        onPreview,
      });
    } catch (error: unknown) {
      throw normalizeMediaPreviewError(error);
    } finally {
      active = false;
    }
  },
  retryUnavailableMedia: (mediaId, onProgress) =>
    invokeImageProcessing<IpcMediaPreview>("retry_unavailable_media", { mediaId }, onProgress).catch(
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

        if (typeof error === "object" && error !== null && "code" in error && error.code === "unfilled_layout_positions" && "layoutProblems" in error) {
          const problems = parseLayoutExportProblems(error.layoutProblems);
          if (problems?.length) throw new LayoutExportBlockedError(problems);
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
