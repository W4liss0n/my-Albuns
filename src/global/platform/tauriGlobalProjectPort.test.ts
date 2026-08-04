import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type {
  NewProjectConfiguration,
  NewProjectCreationConfiguration,
} from "../application/globalProjectPort";
import { tauriGlobalProjectPort } from "./tauriGlobalProjectPort";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

beforeEach(() => {
  vi.mocked(invoke).mockReset();
});

const configuration: NewProjectConfiguration = {
  document: {
    displayUnit: "mm",
    sheetWidthUm: 600_000,
    sheetHeightUm: 300_000,
    dpi: 300,
    bleedUm: 3_000,
    safetyUm: 3_000,
  },
  structure: {
    sheetCount: 2,
    firstSheet: "double",
    lastSheet: "singlePage",
  },
};

const creationConfiguration: NewProjectCreationConfiguration = {
  ...configuration,
  visualDefaults: {
    background: {
      scope: "perSide",
      left: { kind: "color", rgb: "#F4F1EA" },
      right: {
        kind: "image",
        selectionId: "selection-background-right",
      },
    },
    overlay: {
      scope: "bothSides",
      both: { kind: "image", selectionId: "selection-overlay" },
    },
    frameBorder: { kind: "solid", rgb: "#123456", widthUm: 2_000 },
  },
};

test("starts creation with exactly the normalized configuration and no pathname or overwrite authority", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ status: "cancelled" });

  await expect(
    tauriGlobalProjectPort.createProject(creationConfiguration),
  ).resolves.toEqual({ status: "cancelled" });
  expect(invoke).toHaveBeenCalledWith("create_project", {
    configuration: creationConfiguration,
  });
});

test("chooses a provisional decorative without exposing native path data", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    selectionId: "selection-background",
    displayName: "Textura.jpg",
    previewUrl: "http://myalbuns-preview.localhost/selection-background",
    pathname: "C:\\Acervo\\Textura.jpg",
  });

  await expect(
    tauriGlobalProjectPort.chooseProvisionalDecorative(),
  ).resolves.toEqual({
    status: "selected",
    selection: {
      selectionId: "selection-background",
      displayName: "Textura.jpg",
      previewUrl: "http://myalbuns-preview.localhost/selection-background",
    },
  });
  expect(invoke).toHaveBeenCalledWith("choose_provisional_decorative");
});

test("keeps picker cancellation distinct from a malformed response", async () => {
  vi.mocked(invoke)
    .mockResolvedValueOnce(null)
    .mockResolvedValueOnce({
      selectionId: "../native-path",
      displayName: "Textura.jpg",
      previewUrl: "file:///C:/Acervo/Textura.jpg",
    });

  await expect(
    tauriGlobalProjectPort.chooseProvisionalDecorative(),
  ).resolves.toEqual({ status: "cancelled" });
  await expect(
    tauriGlobalProjectPort.chooseProvisionalDecorative(),
  ).resolves.toEqual({
    status: "failed",
    error: {
      code: "decorative_picker_unavailable",
      message: "Não foi possível concluir o seletor de Imagem decorativa.",
      action: "Tente novamente.",
    },
  });
});

test("preserves an actionable typed picker failure", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "unsupported_image",
    message: "O arquivo escolhido não contém uma imagem JPEG ou PNG.",
    action: "Escolha outro arquivo JPEG ou PNG.",
    pathname: "C:\\Acervo\\Texto.txt",
  });

  await expect(
    tauriGlobalProjectPort.chooseProvisionalDecorative(),
  ).resolves.toEqual({
    status: "failed",
    error: {
      code: "unsupported_image",
      message: "O arquivo escolhido não contém uma imagem JPEG ou PNG.",
      action: "Escolha outro arquivo JPEG ou PNG.",
    },
  });
});

test("releases one opaque provisional selection and can clear the registry", async () => {
  vi.mocked(invoke).mockResolvedValue(undefined);

  await tauriGlobalProjectPort.releaseProvisionalDecorative(
    "selection-background",
  );
  await tauriGlobalProjectPort.clearProvisionalDecoratives();

  expect(invoke).toHaveBeenNthCalledWith(
    1,
    "release_provisional_decorative",
    { selectionId: "selection-background" },
  );
  expect(invoke).toHaveBeenNthCalledWith(
    2,
    "clear_provisional_decoratives",
  );
});

test("validates the normalized configuration through the Core boundary", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ errors: [] });

  await expect(
    tauriGlobalProjectPort.validateProjectConfiguration(configuration),
  ).resolves.toEqual({ status: "valid" });
  expect(invoke).toHaveBeenCalledWith("validate_project_configuration", {
    configuration,
  });
});

test("preserves all structured Core validation codes", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    errors: [
      "sheetWidthNotEven",
      "safetyEliminatesSafeArea",
    ],
  });

  await expect(
    tauriGlobalProjectPort.validateProjectConfiguration(configuration),
  ).resolves.toEqual({
    status: "invalid",
    errors: [
      "sheetWidthNotEven",
      "safetyEliminatesSafeArea",
    ],
  });
});

test("turns an unavailable Core validation into an actionable failure", async () => {
  vi.mocked(invoke).mockRejectedValueOnce({
    code: "validation_transport_failed",
    message: "A validação não respondeu.",
    action: "Tente novamente.",
  });

  await expect(
    tauriGlobalProjectPort.validateProjectConfiguration(configuration),
  ).resolves.toEqual({
    status: "failed",
    error: {
      code: "validation_transport_failed",
      message: "A validação não respondeu.",
      action: "Tente novamente.",
    },
  });
});

test("keeps an unavailable creation distinct from an unavailable opening", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("command unavailable"));

  await expect(
    tauriGlobalProjectPort.createProject(creationConfiguration),
  ).resolves.toEqual({
    status: "failed",
    error: {
      code: "create_project_unavailable",
      message: "Não foi possível iniciar a criação do Projeto.",
      action:
        "Tente novamente. Se o problema continuar, reinicie o MyAlbuns.",
    },
  });
});

test.each(["opened", "cancelled"] as const)(
  "opens through the native backend and returns the %s terminal without a pathname",
  async (status) => {
    vi.mocked(invoke).mockResolvedValueOnce({
      status,
      pathname: "C:\\Trabalho\\Álbum.myalbuns",
    });

    await expect(tauriGlobalProjectPort.openProject()).resolves.toEqual({
      status,
    });
    expect(invoke).toHaveBeenCalledWith("open_project");
  },
);

test("keeps an actionable structured backend failure inside the application port", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    status: "failed",
    error: {
      code: "project_in_use",
      stage: "open",
      message: "Este Projeto já está aberto em outra janela.",
      action: "Feche a outra janela e tente novamente.",
      pathname: "C:\\Trabalho\\Álbum.myalbuns",
    },
  });

  await expect(tauriGlobalProjectPort.openProject()).resolves.toEqual({
    status: "failed",
    error: {
      code: "project_in_use",
      stage: "open",
      message: "Este Projeto já está aberto em outra janela.",
      action: "Feche a outra janela e tente novamente.",
    },
  });
});

test("lists only valid recent Project summaries without pathnames", async () => {
  vi.mocked(invoke).mockResolvedValueOnce([
    {
      id: "recent-ana",
      name: "Álbum da Ana",
      pathname: "C:\\Trabalho\\Ana.myalbuns",
    },
    { id: 42, name: "Inválido" },
    null,
  ]);

  await expect(tauriGlobalProjectPort.listRecentProjects()).resolves.toEqual([
    { id: "recent-ana", name: "Álbum da Ana" },
  ]);
  expect(invoke).toHaveBeenCalledWith("recent_projects");
});

test("keeps the welcome surface operational when recent Projects are unavailable", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("state unavailable"));

  await expect(
    tauriGlobalProjectPort.listRecentProjects(),
  ).resolves.toEqual([]);
});

test("reopens a recent Project by opaque id only", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    status: "opened",
    pathname: "C:\\Trabalho\\Ana.myalbuns",
  });

  await expect(
    tauriGlobalProjectPort.openRecentProject("recent-ana"),
  ).resolves.toEqual({ status: "opened" });
  expect(invoke).toHaveBeenCalledWith("open_recent_project", {
    projectId: "recent-ana",
  });
});

test("reads a structured startup failure without its pathname", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({
    code: "invalid_project",
    message: "O arquivo selecionado não é um Projeto válido.",
    action: "Escolha outro arquivo .myalbuns.",
    pathname: "C:\\Trabalho\\Inválido.myalbuns",
  });

  await expect(
    tauriGlobalProjectPort.startupOpenFailure(),
  ).resolves.toEqual({
    code: "invalid_project",
    message: "O arquivo selecionado não é um Projeto válido.",
    action: "Escolha outro arquivo .myalbuns.",
  });
  expect(invoke).toHaveBeenCalledWith("startup_open_failure");
});

test("ignores an unavailable startup diagnostic", async () => {
  vi.mocked(invoke).mockRejectedValueOnce(new Error("startup unavailable"));

  await expect(
    tauriGlobalProjectPort.startupOpenFailure(),
  ).resolves.toBeNull();
});
