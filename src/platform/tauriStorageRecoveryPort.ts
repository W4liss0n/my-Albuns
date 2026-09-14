import { invoke } from "@tauri-apps/api/core";
import type { StorageRecoveryPort } from "../application/storageRecovery";
import type { StorageRecovery } from "./generated/StorageRecovery";

export const tauriStorageRecoveryPort: StorageRecoveryPort = {
  status: owner => invoke<StorageRecovery | null>("storage_recovery_status", { owner }),
  clear: id => invoke<boolean>("clear_storage_recovery_cache", { id }),
};
