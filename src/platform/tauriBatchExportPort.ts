import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { tauriWindowControls } from "./tauriWindowControls";
import type { BatchExportPort } from "../application/batchExport";
import { tauriStorageRecoveryPort } from "./tauriStorageRecoveryPort";
import type { BatchExportProgress } from "./generated/BatchExportProgress";
import type { BatchExportView } from "./generated/BatchExportView";
import type { BatchExportOptions } from "./generated/BatchExportOptions";
import type { BatchRecoverySummary } from "./generated/BatchRecoverySummary";

export const openBatchExport = () => invoke<void>("open_batch_export");
export const tauriBatchExportPort: BatchExportPort = {
  storageRecovery: tauriStorageRecoveryPort,
  current: () => invoke<BatchExportView | null>("batch_current"),
  recoveries: () => invoke<BatchRecoverySummary[]>("batch_recoveries"),
  chooseFolder: () => invoke("batch_choose_folder"),
  countProjects: source => invoke("batch_count_projects", { source }),
  prepare: (options: BatchExportOptions) => invoke<BatchExportView>("batch_prepare", { options }),
  recheck: () => invoke<BatchExportView>("batch_recheck"),
  ignore: itemId => invoke<BatchExportView>("batch_ignore", { itemId }),
  relink: itemId => invoke<BatchExportView | null>("batch_relink", { itemId }),
  openProject: itemId => invoke("batch_open_project", { itemId }),
  run: policy => invoke<BatchExportView>("batch_run", { policy }),
  cancel: () => invoke("batch_cancel"),
  resume: id => invoke<BatchExportView>("batch_resume", { id }),
  end: id => invoke("batch_end", { id }),
  close: () => invoke("close_batch_export"),
  resultReady: async () => {
    const shell = document.querySelector(".ui-owned-window-shell");
    if (shell) await tauriWindowControls.fitContent(() => shell.getBoundingClientRect().height, shell.getBoundingClientRect().width);
    await invoke("batch_result_ready");
  },
  progress: () => invoke<BatchExportProgress | null>("batch_progress"),
  onView: callback => listen<BatchExportView>("myalbuns://batch-view", event => callback(event.payload)),
  onProgress: callback => listen<BatchExportProgress>("myalbuns://batch-progress", event => callback(event.payload)),
};
