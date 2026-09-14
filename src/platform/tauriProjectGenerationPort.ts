import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { GenerationProgress, GenerationView, ProjectGenerationLauncher, ProjectGenerationPort } from "../application/projectGeneration";
import { tauriWindowControls } from "./tauriWindowControls";

export const tauriProjectGenerationLauncher: ProjectGenerationLauncher = { open: () => invoke("open_project_generation") };
export const tauriProjectGenerationPort: ProjectGenerationPort = {
  model: () => invoke("generation_model"), chooseFolder: () => invoke("generation_choose_folder"), count: source => invoke("generation_count", { source }),
  current: () => invoke("generation_current"), prepare: options => invoke("generation_prepare", { options }),
  decide: (id, decision) => invoke("generation_decide", { id, decision }), recheck: () => invoke("generation_recheck"),
  run: () => invoke("generation_run"), cancel: () => invoke("generation_cancel"), close: () => invoke("close_project_generation"),
  progress: () => invoke("generation_progress"),
  onView: callback => listen<GenerationView>("myalbuns://generation-view", event => callback(event.payload)),
  onProgress: callback => listen<GenerationProgress>("myalbuns://generation-progress", event => callback(event.payload)),
  resultReady: async () => {
    const shell = document.querySelector(".ui-owned-window-shell");
    if (shell) await tauriWindowControls.fitContent(() => shell.getBoundingClientRect().height, shell.getBoundingClientRect().width);
    await invoke("generation_result_ready");
  },
};
