import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";

export interface MediaPreview {
  mediaId: string;
  url: string;
}

export interface ExportResult {
  outputPath: string;
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

export interface ProjectSessionPort {
  load(operationId: string): Promise<EditorProjection>;
  apply(intent: ProjectIntent): Promise<EditorProjection>;
  undo(): Promise<EditorProjection>;
  redo(): Promise<EditorProjection>;
}

export interface MediaPreviewPort {
  prepareMediaPreviews(): Promise<readonly MediaPreview[] | null>;
}

export interface ExportPort {
  startPreview(onEvent: (event: ExportProgressEvent) => void): ExportAttempt;
}
