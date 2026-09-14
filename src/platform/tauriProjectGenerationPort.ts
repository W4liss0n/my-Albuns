import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ProjectGenerationPort } from "../application/projectGeneration";
import type { GenerationProgress } from "./generated/GenerationProgress";
import type { GenerationView } from "./generated/GenerationView";
import type { GenerationOptions } from "./generated/GenerationOptions";
import { tauriWindowControls } from "./tauriWindowControls";

export const tauriProjectGenerationPort: ProjectGenerationPort = {
  model: () => invoke("generation_model"), chooseFolder: () => invoke("generation_choose_folder"), count: source => invoke("generation_count", { source }),
  current: () => invoke<GenerationView | null>("generation_current"), prepare: options => invoke<GenerationView>("generation_prepare", { options: options satisfies GenerationOptions }),
  decide: (id, decision) => invoke<GenerationView>("generation_decide", { id, decision }), recheck: () => invoke<GenerationView>("generation_recheck"),
  run: () => invoke<GenerationView>("generation_run"), cancel: () => invoke("generation_cancel"), close: () => invoke("close_project_generation"),
  progress: () => invoke<GenerationProgress | null>("generation_progress"),
  onView: callback => listen<GenerationView>("myalbuns://generation-view", event => callback(event.payload)),
  onProgress: callback => listen<GenerationProgress>("myalbuns://generation-progress", event => callback(event.payload)),
  resultReady: async () => {
    const shell = document.querySelector(".ui-owned-window-shell");
    if (shell) await tauriWindowControls.fitContent(() => shell.getBoundingClientRect().height, shell.getBoundingClientRect().width);
    await invoke("generation_result_ready");
  },
};
