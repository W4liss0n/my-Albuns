import { invoke } from "@tauri-apps/api/core";
import type { ProjectGenerationLauncher } from "../application/projectGeneration";

export const tauriProjectGenerationLauncher: ProjectGenerationLauncher = { open: () => invoke("open_project_generation") };
