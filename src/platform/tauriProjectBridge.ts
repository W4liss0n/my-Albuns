import { invoke } from "@tauri-apps/api/core";

import type {
  EditorProjection,
  ExportResult,
  MediaPreview,
  ProjectBridge,
} from "../domain/project";

interface MediaPreviewCatalogWire {
  previews: Array<{
    mediaId: string;
    url: string;
    widthPx: number;
    heightPx: number;
  }>;
}

export const tauriProjectBridge: ProjectBridge = {
  load: (operationId) =>
    invoke<EditorProjection>("project_state", { operationId }),
  apply: (intent) =>
    invoke<EditorProjection>("apply_project_intent", { intent }),
  undo: () => invoke<EditorProjection>("undo_project"),
  redo: () => invoke<EditorProjection>("redo_project"),
  prepareMediaPreviews: async () => {
    const catalog = await invoke<MediaPreviewCatalogWire | null>(
      "prepare_media_previews",
    );
    if (!catalog) return null;
    return catalog.previews satisfies MediaPreview[];
  },
  exportPreview: () => invoke<ExportResult>("export_spike"),
};
