import { invoke } from "@tauri-apps/api/core";

import type {
  EditorProjection,
  ExportResult,
  ProjectBridge,
} from "../domain/project";

export const tauriProjectBridge: ProjectBridge = {
  load: (operationId) =>
    invoke<EditorProjection>("project_state", { operationId }),
  apply: (intent) =>
    invoke<EditorProjection>("apply_project_intent", { intent }),
  undo: () => invoke<EditorProjection>("undo_project"),
  redo: () => invoke<EditorProjection>("redo_project"),
  exportPreview: () => invoke<ExportResult>("export_spike"),
};
