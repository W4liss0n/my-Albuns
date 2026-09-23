import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ImageViewerWindowPort, ViewerAction, ViewerCorrectionAction, PreparedEyeCorrection } from "../application/imageViewerWindow";

export const tauriImageViewerWindow: ImageViewerWindowPort = {
  open: (presentation) => invoke("open_image_viewer", { presentation }),
  update: (presentation) => invoke("update_image_viewer", { presentation }),
  close: (sessionId) => invoke("close_image_viewer", { sessionId }),
  onNavigate: (callback) => listen<ViewerAction>("myalbuns://image-viewer-navigate", (event) => callback(event.payload)),
  onClosed: (callback) => listen<string>("myalbuns://image-viewer-closed", (event) => callback(event.payload)),
  onCorrection: (callback) => listen<ViewerCorrectionAction>("myalbuns://image-viewer-correction", (event) => callback(event.payload)),
  prepareCorrection: (request) => invoke<PreparedEyeCorrection>("prepare_eye_correction", request),
  cancelCorrection: () => invoke<void>("cancel_eye_correction"),
  applyCorrection: (sessionId, token) => invoke("apply_eye_correction", { sessionId, token }),
};
