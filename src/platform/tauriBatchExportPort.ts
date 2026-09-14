import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { tauriWindowControls } from "./tauriWindowControls";
import type { BatchExportPort, BatchExportProgress, BatchExportView } from "../application/batchExport";

export const openBatchExport = () => invoke<void>("open_batch_export");
export const tauriBatchExportPort: BatchExportPort = {
  current: () => invoke("batch_current"),
  recoveries: () => invoke("batch_recoveries"),
  chooseFolder: () => invoke("batch_choose_folder"),
  countProjects: source => invoke("batch_count_projects", { source }),
  prepare: options => invoke("batch_prepare", { options }),
  recheck: () => invoke("batch_recheck"),
  ignore: itemId => invoke("batch_ignore", { itemId }),
  relink: itemId => invoke("batch_relink", { itemId }),
  openProject: itemId => invoke("batch_open_project", { itemId }),
  run: policy => invoke("batch_run", { policy }),
  cancel: () => invoke("batch_cancel"),
  resume: id => invoke("batch_resume", { id }),
  end: id => invoke("batch_end", { id }),
  close: () => invoke("close_batch_export"),
  resultReady: async () => {
    const shell = document.querySelector(".ui-owned-window-shell");
    if (shell) await tauriWindowControls.fitContent(() => shell.getBoundingClientRect().height, 800);
    await invoke("batch_result_ready");
  },
  progress: () => invoke("batch_progress"),
  onView: callback => listen<BatchExportView>("myalbuns://batch-view", event => callback(event.payload)),
  onProgress: callback => listen<BatchExportProgress>("myalbuns://batch-progress", event => callback(event.payload)),
};
