import { invoke } from "@tauri-apps/api/core";

import type {
  EditorProjection,
  ProjectIntent,
} from "../domain/project";
import type {
  ExportPort,
  ExportResult,
  MediaPreview,
  MediaPreviewPort,
  ProjectSessionPort,
} from "../application/projectPorts";

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
  exportPreview: () => invoke<ExportResult>("export_spike"),
};
