import { invoke } from "@tauri-apps/api/core";
import type { MediaLocationPort } from "../application/mediaLocation";

export const tauriMediaLocationPort: MediaLocationPort = {
  async showInFolder(mediaId) {
    try { await invoke("show_media_in_folder", { mediaId }); }
    catch (error) {
      throw new Error(typeof error === "string" && error.length > 0 ? error : "Não foi possível abrir a pasta da imagem. Tente novamente.");
    }
  },
};
