import { invoke } from "@tauri-apps/api/core";
import { beforeEach, expect, test, vi } from "vitest";

import type { NewProjectConfiguration } from "../application/globalProjectPort";
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

test("starts creation with exactly the normalized configuration and no pathname or overwrite authority", async () => {
  vi.mocked(invoke).mockResolvedValueOnce({ status: "cancelled" });

  await expect(
    tauriGlobalProjectPort.createProject(configuration),
  ).resolves.toEqual({ status: "cancelled" });
  expect(invoke).toHaveBeenCalledWith("create_project", {
    configuration,
  });
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
    tauriGlobalProjectPort.createProject(configuration),
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
