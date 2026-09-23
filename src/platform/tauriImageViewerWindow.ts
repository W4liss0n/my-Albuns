import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ImageViewerWindowPort, ViewerAction } from "../application/imageViewerWindow";

export const tauriImageViewerWindow: ImageViewerWindowPort = {
  open: (presentation) => invoke("open_image_viewer", { presentation }),
  update: (presentation) => invoke("update_image_viewer", { presentation }),
  close: (sessionId) => invoke("close_image_viewer", { sessionId }),
  onNavigate: (callback) => listen<ViewerAction>("myalbuns://image-viewer-navigate", (event) => callback(event.payload)),
  onClosed: (callback) => listen<string>("myalbuns://image-viewer-closed", (event) => callback(event.payload)),
};
