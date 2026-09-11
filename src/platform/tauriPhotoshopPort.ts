import { invoke } from "@tauri-apps/api/core";
import type { PhotoshopPort, PhotoshopSettingsPort, PhotoshopStatus, PhotoshopErrorCode } from "../application/photoshop";
import { PhotoshopError } from "../application/photoshop";
import { isIpcRecord } from "./ipcGuards";
import type { PhotoshopStatus as NativePhotoshopStatus } from "./generated/PhotoshopStatus";
import type { PhotoshopPhotoTarget as NativePhotoshopPhotoTarget } from "./generated/PhotoshopPhotoTarget";

const errorCodes: ReadonlySet<string> = new Set([
  "installation_unavailable", "invalid_installation", "original_absent", "original_unavailable",
  "invalid_context", "launch_failed", "store_unavailable", "dialog_unavailable",
] as const);

async function call(command: string, args?: Record<string, unknown>): Promise<unknown> {
  try { return await invoke(command, args); }
  catch (error) {
    if (isIpcRecord(error) && typeof error.code === "string" && errorCodes.has(error.code) && typeof error.message === "string") {
      throw new PhotoshopError(error.code as PhotoshopErrorCode, error.message);
    }
    throw new PhotoshopError("invalid_response", "Não foi possível concluir a operação do Photoshop. Tente novamente.");
  }
}

export function parsePhotoshopStatus(value: unknown): PhotoshopStatus {
  if (!isIpcRecord(value) || !Number.isSafeInteger(value.revision) || (value.revision as number) < 0 || !Array.isArray(value.installations)) {
    throw new PhotoshopError("invalid_response", "Não foi possível consultar as instalações do Photoshop.");
  }
  const installations = value.installations.map((item: unknown) => {
    if (!isIpcRecord(item) || ![item.id, item.name, item.version, item.path].every((part) => typeof part === "string" && part.length > 0)) {
      throw new PhotoshopError("invalid_response", "A lista de instalações do Photoshop é inválida.");
    }
    return { id: item.id as string, name: item.name as string, version: item.version as string, path: item.path as string };
  });
  const selected = value.selectedInstallationId;
  if (new Set(installations.map((item) => item.id)).size !== installations.length ||
      (selected !== null && (typeof selected !== "string" || !installations.some((item) => item.id === selected)))) {
    throw new PhotoshopError("invalid_response", "A instalação selecionada do Photoshop é inválida.");
  }
  return { revision: value.revision as number, installations, selectedInstallationId: selected as string | null } satisfies NativePhotoshopStatus;
}

const status = async () => parsePhotoshopStatus(await call("photoshop_status"));

export const tauriPhotoshopPort: PhotoshopPort = {
  status,
  async openPhoto(target) {
    const nativeTarget: NativePhotoshopPhotoTarget = target.kind === "panel"
      ? { kind: "panel", mediaIds: [...target.mediaIds] } : { kind: "frames", frameIds: [...target.frameIds] };
    await call("open_in_photoshop", { target: nativeTarget });
  },
  async openSettings(section) { await call("open_application_settings", { section }); },
};

export const tauriPhotoshopSettingsPort: PhotoshopSettingsPort = {
  status,
  async select(installationId) { return parsePhotoshopStatus(await call("select_photoshop", { installationId })); },
  async locate() {
    const value = await call("choose_photoshop");
    return value === null ? null : parsePhotoshopStatus(value);
  },
};
