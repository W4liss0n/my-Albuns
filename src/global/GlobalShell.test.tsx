import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type {
  GlobalProjectPort,
  OpenProjectFailure,
  OpenProjectOutcome,
} from "./application/globalProjectPort";
import { GlobalShell } from "./GlobalShell";

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
    createProject: async () => ({ status: "cancelled" }),
    openProject: async () => ({ status: "cancelled" }),
    listRecentProjects: async () => [],
    openRecentProject: async () => ({ status: "cancelled" }),
    startupOpenFailure: async () => null,
    ...overrides,
  };
}

test("shows the global welcome surface without a Project workspace", () => {
  const projectPort = createProjectPort({ openProject: vi.fn() });

  render(<GlobalShell projectPort={projectPort} />);

  expect(
    screen.getByRole("heading", { name: "Projetos recentes" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Novo Projeto" }),
  ).toBeEnabled();
  expect(
    screen.getByRole("button", { name: "Abrir Projeto" }),
  ).toBeEnabled();
  expect(screen.queryByTestId("album-canvas")).not.toBeInTheDocument();
});

test("opens and cancels the creation assistant without reaching the native port", async () => {
  const user = userEvent.setup();
  const createProject = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <GlobalShell projectPort={createProjectPort({ createProject })} />,
  );

  await user.click(screen.getByRole("button", { name: "Novo Projeto" }));
  expect(screen.getByRole("dialog")).toHaveAccessibleName("Dimensões");

  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(createProject).not.toHaveBeenCalled();
});

test("starts one opening attempt and reports that it is in progress", async () => {
  const user = userEvent.setup();
  const opening = deferred<OpenProjectOutcome>();
  const openProject = vi.fn(() => opening.promise);

  render(<GlobalShell projectPort={createProjectPort({ openProject })} />);

  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

  expect(openProject).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("button", { name: "Abrindo Projeto…" }),
  ).toBeDisabled();
  expect(screen.getByRole("status")).toHaveTextContent(
    "Preparando a Janela do Projeto",
  );
});

test("shows an actionable structured failure without exposing a pathname", async () => {
  const user = userEvent.setup();
  const openProject = vi.fn(async () => ({
    status: "failed" as const,
    error: {
      code: "project_in_use",
      message: "Este Projeto já está aberto em outra janela.",
      action: "Feche a outra janela e tente novamente.",
    },
  }));

  render(<GlobalShell projectPort={createProjectPort({ openProject })} />);
  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Este Projeto já está aberto em outra janela.",
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Feche a outra janela e tente novamente.",
  );
  expect(
    screen.getByRole("button", { name: "Tentar novamente" }),
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
      projectPort={createProjectPort({ listRecentProjects })}
    />,
  );

  expect(
    await screen.findByRole("button", { name: "Álbum da Ana" }),
  ).toBeEnabled();
  expect(
    screen.getByRole("button", { name: "Álbum da Bia" }),
  ).toBeEnabled();
  expect(listRecentProjects).toHaveBeenCalledOnce();
});

test("reopens a recent Project using only its opaque id", async () => {
  const user = userEvent.setup();
  const opening = deferred<OpenProjectOutcome>();
  const openProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const openRecentProject = vi.fn(() => opening.promise);

  render(
    <GlobalShell
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
  expect(screen.getByRole("status")).toHaveTextContent(
    "Preparando a Janela do Projeto",
  );
});

test("shows the startup failure from a direct Windows opening", async () => {
  const startupOpenFailure = vi.fn(async () => ({
    code: "invalid_project",
    message: "O arquivo selecionado não é um Projeto válido.",
    action: "Escolha outro arquivo .myalbuns.",
  }));

  render(
    <GlobalShell
      projectPort={createProjectPort({ startupOpenFailure })}
    />,
  );

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "O arquivo selecionado não é um Projeto válido.",
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Escolha outro arquivo .myalbuns.",
  );
  expect(startupOpenFailure).toHaveBeenCalledOnce();
});

test("does not overwrite a newer opening attempt with a late startup failure", async () => {
  const user = userEvent.setup();
  const startupFailure = deferred<OpenProjectFailure | null>();

  render(
    <GlobalShell
      projectPort={createProjectPort({
        openProject: async () => ({ status: "cancelled" }),
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

  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});
