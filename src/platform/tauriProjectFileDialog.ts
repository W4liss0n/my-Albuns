import { open } from "@tauri-apps/plugin-dialog";

import type { ProjectFileDialog } from "../application/projectFileDialog";

export const tauriProjectFileDialog: ProjectFileDialog = {
  openProjectFile() {
    return open({
      directory: false,
      multiple: false,
      title: "Abrir Projeto",
    });
  },
};
