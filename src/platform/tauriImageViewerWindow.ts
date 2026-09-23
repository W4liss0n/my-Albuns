import { invoke, type InvokeArgs } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ImageViewerWindowPort, ViewerAction, ViewerCorrectionAction, PreparedEyeCorrection } from "../application/imageViewerWindow";

async function invokeCorrection<T>(command: string, args: InvokeArgs, fallback: string): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error: unknown) {
    if (error instanceof Error) throw error;
    // Rust Result errors arrive as serialized strings, not JavaScript Errors.
    throw new Error(typeof error === "string" && error.trim() ? error : fallback);
  }
}

export const tauriImageViewerWindow: ImageViewerWindowPort = {
  open: (presentation) => invoke("open_image_viewer", { presentation }),
  update: (presentation) => invoke("update_image_viewer", { presentation }),
  close: (sessionId) => invoke("close_image_viewer", { sessionId }),
  onNavigate: (callback) => listen<ViewerAction>("myalbuns://image-viewer-navigate", (event) => callback(event.payload)),
  onClosed: (callback) => listen<string>("myalbuns://image-viewer-closed", (event) => callback(event.payload)),
  onCorrection: (callback) => listen<ViewerCorrectionAction>("myalbuns://image-viewer-correction", (event) => callback(event.payload)),
  prepareCorrection: (request) => invokeCorrection<PreparedEyeCorrection>("prepare_eye_correction", request, "Não foi possível corrigir os olhos."),
  cancelCorrection: () => invoke<void>("cancel_eye_correction"),
  applyCorrection: (sessionId, token) => invokeCorrection("apply_eye_correction", { sessionId, token }, "Não foi possível salvar a correção."),
};
