export interface MediaLocationPort {
  /** Opens the folder of the medium's original file with that file selected. */
  showInFolder(mediaId: string): Promise<void>;
}

export function mediaLocationErrorMessage(error: unknown): string {
  return error instanceof Error ? error.message : "Não foi possível abrir a pasta da imagem. Tente novamente.";
}
