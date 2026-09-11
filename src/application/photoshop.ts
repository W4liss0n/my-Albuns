export interface PhotoshopInstallation {
  id: string;
  name: string;
  version: string;
  path: string;
}

export interface PhotoshopStatus {
  revision: number;
  installations: readonly PhotoshopInstallation[];
  selectedInstallationId: string | null;
}

export type SettingsSection = "performance" | "photoshop";
export type PhotoshopPhotoTarget =
  | { kind: "panel"; mediaIds: readonly string[] }
  | { kind: "frames"; frameIds: readonly string[] };

export type PhotoshopErrorCode =
  | "installation_unavailable" | "invalid_installation" | "original_absent"
  | "original_unavailable" | "invalid_context" | "launch_failed"
  | "store_unavailable" | "dialog_unavailable" | "invalid_response";

export class PhotoshopError extends Error {
  constructor(readonly code: PhotoshopErrorCode, message: string) {
    super(message);
    this.name = "PhotoshopError";
  }
}

export interface PhotoshopPort {
  status(): Promise<PhotoshopStatus>;
  openPhoto(target: PhotoshopPhotoTarget): Promise<void>;
  openSettings(section: SettingsSection): Promise<void>;
}

export interface PhotoshopSettingsPort {
  status(): Promise<PhotoshopStatus>;
  select(installationId: string): Promise<PhotoshopStatus>;
  locate(): Promise<PhotoshopStatus | null>;
}

export function photoshopErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Não foi possível concluir a operação. Tente novamente.";
}
