import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ViewerCorrectionAction, ViewerPresentation } from "../../application/imageViewerWindow";

export const tauriImageViewerClient = {
  current: () => invoke<ViewerPresentation | null>("current_image_viewer"),
  onPresentation: (callback: (presentation: ViewerPresentation) => void) =>
    listen<ViewerPresentation>("myalbuns://image-viewer-presentation", (event) => callback(event.payload)),
  ready: (token: number) => invoke<void>("owned_window_content_ready", { token }),
  navigate: (sessionId: string, offset: -1 | 1) => invoke<void>("navigate_image_viewer", { sessionId, offset }),
  close: (sessionId: string) => invoke<void>("close_image_viewer", { sessionId }),
  correction: (action: ViewerCorrectionAction) => invoke<void>("act_image_viewer_correction", { action }),
};
