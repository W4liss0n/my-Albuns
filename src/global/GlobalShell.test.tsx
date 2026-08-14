import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type {
  GlobalProjectPort,
  NewProjectPort,
  OpenProjectFailure,
  OpenProjectOutcome,
} from "./application/globalProjectPort";
import type { GraphicsDiagnostic } from "../application/graphics";
import { GlobalShell } from "./GlobalShell";

const supportedGraphics: GraphicsDiagnostic = {
  supported: true,
  renderer: "NVIDIA GeForce RTX",
  reason: "WebGL2 acelerado por hardware confirmado.",
  limits: {
    maxTextureSizePx: 16_384,
    maxRenderbufferSizePx: 16_384,
    maxTextureImageUnits: 16,
  },
};

const unavailableGraphics: GraphicsDiagnostic = {
  supported: false,
  code: "webgl2_unavailable",
  renderer: "indisponível",
  reason: "WebGL2 acelerado por hardware não foi confirmado.",
  limits: null,
};

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

function createProjectPort(
  overrides: Partial<GlobalProjectPort> = {},
): GlobalProjectPort {
  return {
    completeGraphicsGate: async () => null,
    openProject: async () => ({ status: "cancelled" }),
    listRecentProjects: async () => [],
    openRecentProject: async () => ({ status: "cancelled" }),
    startupOpenFailure: async () => null,
    showLaunchFailure: async () => undefined,
    ...overrides,
  };
}

function createNewProjectPort(
  overrides: Partial<NewProjectPort> = {},
): NewProjectPort {
  return {
    chooseProvisionalDecorative: async () => ({ status: "cancelled" }),
    createProject: async () => ({ status: "cancelled" }),
    releaseProvisionalDecorative: async () => undefined,
    validateProjectConfiguration: async () => ({ status: "valid" }),
    ...overrides,
  };
}

test("shows the global welcome surface without a Project workspace", () => {
  const projectPort = createProjectPort({ openProject: vi.fn() });

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={projectPort}
    />,
  );

  expect(
    screen.getByRole("heading", { name: "Projetos recentes" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Novo Projeto" }),
  ).toBeEnabled();
  expect(
    screen.getByRole("button", { name: "Abrir Projeto" }),
  ).toBeEnabled();
  expect(
    screen.getByRole("button", {
      name: "Exportar vários Álbuns de uma vez",
    }),
  ).toBeDisabled();
  expect(screen.queryByTestId("album-canvas")).not.toBeInTheDocument();
});

test("blocks Project hosts at the global graphics boundary when hardware WebGL2 is unavailable", async () => {
  const completeGraphicsGate = vi.fn(async () => null);
  const openProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const createProject = vi.fn(async () => ({ status: "opened" as const }));

  render(
    <GlobalShell
      graphicsDiagnostic={unavailableGraphics}
      newProjectPort={createNewProjectPort({ createProject })}
      projectPort={createProjectPort({
        completeGraphicsGate,
        openProject,
      })}
    />,
  );

  expect(
    await screen.findByRole("heading", { name: "Boas-vindas" }),
  ).toBeInTheDocument();
  expect(
    screen.getByText("WebGL2 acelerado por hardware não foi confirmado."),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Novo Projeto" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Abrir Projeto" }),
  ).not.toBeInTheDocument();
  expect(completeGraphicsGate).toHaveBeenCalledWith(false);
  expect(openProject).not.toHaveBeenCalled();
  expect(createProject).not.toHaveBeenCalled();
});

test("replaces welcome with New Project in the same window and restores welcome on cancel", async () => {
  const user = userEvent.setup();

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={createProjectPort()}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Novo Projeto" }));

  expect(
    screen.getByRole("heading", { level: 1, name: "Novo Projeto" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("heading", { name: "Projetos recentes" }),
  ).not.toBeInTheDocument();
  expect(screen.getAllByRole("banner", { name: "Barra da janela" })).toHaveLength(1);

  await user.click(screen.getByRole("button", { name: "Cancelar" }));

  expect(
    screen.getByRole("heading", { name: "Projetos recentes" }),
  ).toBeInTheDocument();
});

test("keeps opening progress out of the welcome document", async () => {
  const user = userEvent.setup();
  const opening = deferred<OpenProjectOutcome>();
  const openProject = vi.fn(() => opening.promise);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={createProjectPort({ openProject })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

  expect(openProject).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("button", {
      name: "Abrindo Projeto…",
    }),
  ).toBeDisabled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    document.querySelector(".global-primary-actions [role='status']"),
  ).not.toBeInTheDocument();
});

test("shows an actionable structured failure without exposing a pathname", async () => {
  const user = userEvent.setup();
  const showLaunchFailure = vi.fn(async () => undefined);
  const failure = {
    code: "project_in_use",
    message: "Este Projeto já está aberto em outra janela.",
    action: "Feche a outra janela e tente novamente.",
  };
  const openProject = vi.fn(async () => ({
    status: "failed" as const,
    error: failure,
  }));

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={createProjectPort({ openProject, showLaunchFailure })}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

  expect(showLaunchFailure).toHaveBeenCalledWith(failure);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Abrir Projeto" }),
  ).toBeEnabled();
  expect(screen.queryByText(/\.(?:myalbuns)|\\|:\//i)).not.toBeInTheDocument();
});

test("loads and renders recent Projects by name", async () => {
  const listRecentProjects = vi.fn(async () => [
    { id: "recent-ana", name: "Álbum da Ana" },
    { id: "recent-bia", name: "Álbum da Bia" },
  ]);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={createProjectPort({ listRecentProjects })}
    />,
  );

  expect(
    await screen.findByRole("button", { name: "Álbum da Ana" }),
  ).toBeEnabled();
  expect(
    screen.getByRole("button", { name: "Álbum da Bia" }),
  ).toBeEnabled();
  expect(screen.getAllByText("Aberto recentemente")).toHaveLength(2);
  expect(listRecentProjects).toHaveBeenCalledOnce();
});

test("reopens a recent Project using only its opaque id", async () => {
  const user = userEvent.setup();
  const opening = deferred<OpenProjectOutcome>();
  const openProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const openRecentProject = vi.fn(() => opening.promise);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={createProjectPort({
        listRecentProjects: async () => [
          { id: "recent-ana", name: "Álbum da Ana" },
        ],
        openProject,
        openRecentProject,
      })}
    />,
  );

  const recentProject = await screen.findByRole("button", {
    name: "Álbum da Ana",
  });
  await user.click(recentProject);

  expect(openRecentProject).toHaveBeenCalledWith("recent-ana");
  expect(openProject).not.toHaveBeenCalled();
  expect(recentProject).toBeDisabled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
});

test("shows the startup failure from a direct Windows opening", async () => {
  const showLaunchFailure = vi.fn(async () => undefined);
  const startupOpenFailure = vi.fn(async () => ({
    code: "invalid_project",
    message: "O arquivo selecionado não é um Projeto válido.",
    action: "Escolha outro arquivo .myalbuns.",
  }));

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={createProjectPort({
        showLaunchFailure,
        startupOpenFailure,
      })}
    />,
  );

  await waitFor(() => {
    expect(showLaunchFailure).toHaveBeenCalledWith({
      code: "invalid_project",
      message: "O arquivo selecionado não é um Projeto válido.",
      action: "Escolha outro arquivo .myalbuns.",
    });
  });
  expect(startupOpenFailure).toHaveBeenCalledOnce();
});

test("does not overwrite a newer opening attempt with a late startup failure", async () => {
  const user = userEvent.setup();
  const startupFailure = deferred<OpenProjectFailure | null>();
  const showLaunchFailure = vi.fn(async () => undefined);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPort()}
      projectPort={createProjectPort({
        openProject: async () => ({ status: "cancelled" }),
        showLaunchFailure,
        startupOpenFailure: () => startupFailure.promise,
      })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));
  await act(async () => {
    startupFailure.resolve({
      code: "stale_startup_failure",
      message: "Esta falha pertence à tentativa inicial.",
    });
  });

  expect(showLaunchFailure).not.toHaveBeenCalled();
});
