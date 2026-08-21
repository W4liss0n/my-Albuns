import type {
  EditorProjection,
  PhotoDropTarget,
  PhotoPlacementMode,
  ProjectIntent,
  ProjectMutationOutcome,
} from "../domain/project";

export type MediaPreviewState =
  | "ready"
  | "absent"
  | "unavailable"
  | "cache_unavailable";

export interface MediaPreview {
  mediaId: string;
  state: MediaPreviewState;
  url: string | null;
}

export interface MediaPreviewDemand {
  visibleMediaIds: readonly string[];
  preloadMediaIds: readonly string[];
}

export interface MediaPreviewRequest extends MediaPreviewDemand {
  revision: number;
}

export interface CacheProcessorWarning {
  state: "suspended";
  message: string;
}

export type MediaPreviewErrorCode =
  | "unavailable"
  | "unsupported_image"
  | "read_failed";

export class MediaPreviewError extends Error {
  constructor(
    readonly code: MediaPreviewErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "MediaPreviewError";
  }
}

export interface ExportResult {
  widthPx: number;
  heightPx: number;
}

export type ExportProgressStage =
  | "preparing"
  | "loading_sources"
  | "composing"
  | "encoding_output"
  | "verifying"
  | "publishing"
  | "completed";

export type ExportProgressUnits =
  | {
      kind: "unmeasured";
    }
  | {
      kind: "measured";
      completedUnits: number;
      totalUnits: number;
    };

export type ExportProgressEvent =
  | {
      event: "started";
      cancellable: boolean;
    }
  | {
      event: "progress";
      stage: ExportProgressStage;
      units: ExportProgressUnits;
      cancellable: boolean;
    };

export type ExportOutcome =
  | {
      status: "completed";
      result: ExportResult;
    }
  | {
      status: "cancelled";
    };

export type ExportCancelStatus =
  | "requested"
  | "already_requested"
  | "too_late"
  | "not_found";

export interface ExportAttempt {
  completion: Promise<ExportOutcome>;
  cancel(): Promise<ExportCancelStatus>;
}

export type SaveProjectOutcome =
  | { kind: "saved"; revision: number }
  | { kind: "alreadyCurrent"; revision: number };

export interface SaveProjectResult {
  outcome: SaveProjectOutcome;
  projection: EditorProjection;
}

export type SaveProjectFailureCode =
  | "stale_revision"
  | "persisted_baseline_conflict"
  | "save_state_indeterminate"
  | "session_unavailable"
  | "not_found"
  | "unavailable"
  | "access_denied"
  | "invalid_path"
  | "unexpected_object_type"
  | "conflict"
  | "io_failure";

export type SaveProjectErrorCode =
  | SaveProjectFailureCode
  | "invalid_response"
  | "save_unavailable";

export interface SaveProjectErrorContext {
  expected: number;
  current: number;
}

export class SaveProjectError extends Error {
  constructor(
    readonly code: SaveProjectErrorCode,
    message: string,
    readonly context?: SaveProjectErrorContext,
  ) {
    super(message);
    this.name = "SaveProjectError";
  }
}

export type ProjectCloseChoice =
  | "saveAndClose"
  | "discardAndClose"
  | "cancel";

export type ProjectCloseRequestOutcome =
  | { kind: "closed" }
  | { kind: "confirmationRequired" };

export type ProjectCloseResolution =
  | { kind: "closed" }
  | { kind: "cancelled"; projection: EditorProjection };

export type ProjectCloseErrorCode = SaveProjectErrorCode | "close_unavailable";

export class ProjectCloseError extends Error {
  constructor(
    readonly code: ProjectCloseErrorCode,
    message: string,
  ) {
    super(message);
    this.name = "ProjectCloseError";
  }
}

export interface ProjectWindowPort {
  onCloseRequested(listener: () => void): Promise<() => void>;
  requestClose(): Promise<ProjectCloseRequestOutcome>;
  resolveClose(choice: ProjectCloseChoice): Promise<ProjectCloseResolution>;
}

export interface ProjectStartupPort {
  confirmUiReady(): Promise<void>;
}

export interface ProjectCorePort {
  load(operationId: string): Promise<EditorProjection>;
  apply(intent: ProjectIntent): Promise<EditorProjection>;
  applyWithOutcome?(intent: ProjectIntent): Promise<ProjectMutationOutcome>;
  importPhoto?(): Promise<
    | { kind: "cancelled"; projection: EditorProjection }
    | { kind: "imported"; projection: EditorProjection; mediaId: string }
  >;
  resolvePhotoDropTarget?(
    sheetId: string,
    xUm: number,
    yUm: number,
    mode: PhotoPlacementMode,
  ): Promise<PhotoDropTarget>;
  relink(mediaId: string): Promise<EditorProjection>;
  undo(): Promise<EditorProjection>;
  redo(): Promise<EditorProjection>;
  save(expectedRevision: number): Promise<SaveProjectResult>;
}

export interface MediaPreviewPort {
  prepareMediaPreviews(
    demand: MediaPreviewRequest,
  ): Promise<readonly MediaPreview[] | null>;
  retryUnavailableMedia(mediaId: string): Promise<MediaPreview>;
  onMediaChanged(
    listener: (mediaIds: readonly string[]) => void,
  ): Promise<() => void>;
  onCacheProcessorWarning(
    listener: (warning: CacheProcessorWarning) => void,
  ): Promise<() => void>;
}

export interface ExportSheetSelection {
  projectName: string;
  sheetId: string;
  sheetNumber: number;
}

export interface ExportPipelinePort {
  startSheet(
    selection: ExportSheetSelection,
    onEvent: (event: ExportProgressEvent) => void,
  ): ExportAttempt;
}
