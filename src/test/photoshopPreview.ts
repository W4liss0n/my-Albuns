import { PhotoshopError, type PhotoshopPort, type PhotoshopSettingsPort, type PhotoshopStatus } from "../application/photoshop";

export function photoshopSettingsPreview(state: string | null): PhotoshopSettingsPort {
  let status: PhotoshopStatus = state === "absent" ? { revision: 0, installations: [], selectedInstallationId: null } : {
    revision: 1, selectedInstallationId: "2026", installations: [
      { id: "2026", name: "Adobe Photoshop 2026", version: "27.10", path: "C:\\Program Files\\Adobe\\Adobe Photoshop 2026\\Photoshop.exe" },
      { id: "2025", name: "Adobe Photoshop 2025", version: "26.8", path: "D:\\Aplicativos de fotografia e edição de álbuns\\Adobe\\Adobe Photoshop 2025\\Photoshop.exe" },
    ],
  };
  return {
    status: async () => status,
    select: async (id) => (status = { ...status, revision: status.revision + 1, selectedInstallationId: id }),
    locate: async () => { throw new PhotoshopError("invalid_installation", "O arquivo selecionado não é uma instalação válida do Adobe Photoshop. Escolha Photoshop.exe."); },
  };
}

export function photoshopProjectPreview(state: string | null): PhotoshopPort {
  return {
    status: photoshopSettingsPreview(state).status,
    openSettings: async () => { window.location.href = "/settings-preview.html?section=photoshop"; },
    openPhoto: async () => {
      if (state === "failure") throw new PhotoshopError("installation_unavailable", "A instalação selecionada do Photoshop não está mais disponível. Escolha outra instalação em Configurações.");
      document.body.dataset.photoshopOpened = "true";
    },
  };
}
