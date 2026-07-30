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
  exportPreview(): Promise<ExportResult>;
}
