import { invoke } from "@tauri-apps/api/core";
import type { ProjectLauncher } from "../application/projectLauncher";

export const tauriProjectLauncher: ProjectLauncher = {
  newProject: () => invoke("new_project_from_editor"),
  openProject: () => invoke("open_project_from_editor"),
};
