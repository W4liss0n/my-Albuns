import { invoke } from "@tauri-apps/api/core";
import type { CacheSettingsPort } from "../application/cacheSettings";
import { isIpcRecord } from "./ipcGuards";

const invalid = () => new Error("O serviço de Cache retornou uma resposta inválida.");
const bytes = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
function freed(value: unknown): { freedBytes: number } {
  if (!isIpcRecord(value) || !bytes(value.freedBytes)) throw invalid();
  return { freedBytes: value.freedBytes };
}

export const tauriCacheSettingsPort: CacheSettingsPort = {
  async status() {
    const value: unknown = await invoke("cache_service_status");
    if (!isIpcRecord(value) || !bytes(value.occupiedBytes) || !bytes(value.releasableBytes) ||
      value.releasableBytes > value.occupiedBytes || typeof value.clearAllScheduled !== "boolean") throw invalid();
    return { occupiedBytes: value.occupiedBytes, releasableBytes: value.releasableBytes, clearAllScheduled: value.clearAllScheduled };
  },
  async freeClosedProjects() { return freed(await invoke("free_closed_project_cache")); },
  async clearAll() {
    const value: unknown = await invoke("clear_all_cache");
    if (isIpcRecord(value)) {
      if (value.kind === "scheduled") return { kind: "scheduled" };
      if (value.kind === "cleared") return { kind: "cleared", result: freed(value.result) };
    }
    throw invalid();
  },
};
