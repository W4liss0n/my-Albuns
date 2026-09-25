import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import type {
  GlobalProjectPort,
  OpenProjectFailure,
  OpenProjectOutcome,
  ProjectFailureDialogPort,
} from "./application/globalProjectPort";
import type { GraphicsDiagnostic } from "../application/graphics";
import { GlobalShell as ProductGlobalShell } from "./GlobalShell";
import { createNewProjectPortStub } from "./testing/newProjectPortStub";
import { welcomeDatesNow, welcomePreviewRecentProjects } from "../test/welcomePreviewFixtures";

type GlobalShellProps = ComponentProps<typeof ProductGlobalShell>;

function GlobalShell({
  failureDialogPort = { present: async () => undefined },
  newProjectPort = createNewProjectPortStub(),
  ...props
}: Omit<GlobalShellProps, "failureDialogPort" | "newProjectPort"> &
  Partial<Pick<GlobalShellProps, "failureDialogPort" | "newProjectPort">>) {
  return (
    <ProductGlobalShell
      {...props}
      failureDialogPort={failureDialogPort}
      newProjectPort={newProjectPort}
    />
  );
}

const supportedGraphics: GraphicsDiagnostic = {
  supported: true,
  renderer: "NVIDIA GeForce RTX",
  reason: "A aceleração gráfica está disponível.",
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
    onActivationTerminal: async () => () => undefined,
    completeGraphicsGate: async () => null,
    openProject: async () => ({ status: "cancelled" }),
    listRecentProjects: async () => [],
    setRecentProjectFavorite: async () => ({ status: "saved", projects: [] }),
    firstRecentProjectSheet: async () => null,
    openRecentProject: async () => ({ status: "cancelled" }),
    startupOpenFailure: async () => null,
    ...overrides,
  };
}

test("shows the global welcome surface without a Project workspace", () => {
  const projectPort = createProjectPort({ openProject: vi.fn() });
  const openBatch = vi.fn(async () => undefined);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={projectPort}
      onOpenBatch={openBatch}
    />,
  );

  expect(
    screen.getByRole("status", { name: "Nenhum projeto recente" }),
  ).toBeInTheDocument();
  const newProjectButton = screen.getByRole("button", {
    name: "Novo projeto",
  });
  const openProjectButton = screen.getByRole("button", {
    name: "Abrir projeto",
  });
  expect(newProjectButton).toBeEnabled();
  expect(newProjectButton).toHaveAttribute("aria-keyshortcuts", "Control+N");
  expect(openProjectButton).toBeEnabled();
  expect(openProjectButton).toHaveAttribute("aria-keyshortcuts", "Control+O");
  const batchExportPlaceholder = screen.getByRole("button", {
    name: "Exportação em lote",
  });
  expect(batchExportPlaceholder).toBeEnabled();
  fireEvent.click(batchExportPlaceholder);
  expect(openBatch).toHaveBeenCalledOnce();
  expect(screen.queryByTestId("album-canvas")).not.toBeInTheDocument();
});

test("uses the canonical text-only empty state for recent Projects", async () => {
  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort()}
    />,
  );

  const emptyState = await screen.findByRole("status", {
    name: "Nenhum projeto recente",
  });
  expect(emptyState).toHaveTextContent(
    "Crie um projeto ou abra um arquivo .myalbuns.",
  );
  expect(emptyState.querySelector(".ui-empty-state__icon")).toBeNull();
});

test("activates the Windows shortcuts displayed on welcome", async () => {
  const user = userEvent.setup();
  const openProject = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({ openProject })}
    />,
  );

  fireEvent.keyDown(window, { ctrlKey: true, key: "n" });
  expect(
    screen.getByRole("banner", { name: "Barra da janela" }),
  ).toHaveTextContent("Novo projeto");

  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  fireEvent.keyDown(window, { ctrlKey: true, key: "o" });

  await waitFor(() => expect(openProject).toHaveBeenCalledOnce());
});

test("transfers keyboard focus into New Project and restores its trigger on cancel", async () => {
  const user = userEvent.setup();

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort()}
    />,
  );

  fireEvent.keyDown(window, { ctrlKey: true, key: "n" });

  const currentStep = screen
    .getByRole("list", { name: "Etapas da criação" })
    .querySelector<HTMLElement>('[aria-current="step"]');
  expect(currentStep).not.toBeNull();
  expect(currentStep).toHaveFocus();

  const cancel = screen.getByRole("button", { name: "Cancelar" });
  cancel.focus();
  await user.keyboard("{Enter}");

  expect(screen.getByRole("button", { name: "Novo projeto" })).toHaveFocus();
});

test("blocks Project hosts at the global graphics boundary when hardware WebGL2 is unavailable", async () => {
  const completeGraphicsGate = vi.fn(async () => null);
  const openProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const createProject = vi.fn(async () => ({ status: "opened" as const }));

  render(
    <GlobalShell
      graphicsDiagnostic={unavailableGraphics}
      newProjectPort={createNewProjectPortStub({ createProject })}
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
    screen.queryByRole("button", { name: "Novo projeto" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Abrir projeto" }),
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
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort()}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Novo projeto" }));

  const windowTitlebar = screen.getByRole("banner", {
    name: "Barra da janela",
  });
  expect(screen.getAllByText("Novo projeto")).toHaveLength(1);
  expect(windowTitlebar).toContainElement(screen.getByText("Novo projeto"));
  expect(
    screen.queryByRole("heading", { level: 1, name: "Novo projeto" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("heading", { name: "Projetos recentes" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getAllByRole("banner", { name: "Barra da janela" }),
  ).toHaveLength(1);

  await user.click(screen.getByRole("button", { name: "Cancelar" }));

  expect(
    screen.getByRole("status", { name: "Nenhum projeto recente" }),
  ).toHaveClass("global-empty-state");
});

test("routes New Project operational failures through its owned native dialog port", async () => {
  const user = userEvent.setup();
  const error = {
    code: "validation_unavailable",
    message: "A validação está indisponível.",
    action: "Tente novamente.",
  };
  const present = vi.fn(async () => undefined);
  const failureDialogPort: ProjectFailureDialogPort = { present };

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      failureDialogPort={failureDialogPort}
      newProjectPort={createNewProjectPortStub({
        validateProjectConfiguration: async () => ({
          status: "failed",
          error,
        }),
      })}
      projectPort={createProjectPort()}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Novo projeto" }));
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  await waitFor(() =>
    expect(present).toHaveBeenCalledWith({
      context: "configurationValidation",
      error,
    }),
  );
  expect(present).toHaveBeenCalledOnce();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Configurações" }),
  ).toBeInTheDocument();
});

test("keeps opening progress out of the welcome document", async () => {
  const user = userEvent.setup();
  const opening = deferred<OpenProjectOutcome>();
  const openProject = vi.fn(() => opening.promise);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({ openProject })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Abrir projeto" }));

  expect(openProject).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("button", {
      name: "Abrindo projeto…",
    }),
  ).toBeDisabled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    document.querySelector(".global-primary-actions [role='status']"),
  ).not.toBeInTheDocument();
});

test("shows an actionable structured failure without exposing a pathname", async () => {
  const user = userEvent.setup();
  const present = vi.fn(async () => undefined);
  const failure = {
    code: "project_in_use",
    message: "Este projeto já está aberto em outra janela.",
    action: "Feche a outra janela e tente novamente.",
  };
  const openProject = vi.fn(async () => ({
    status: "failed" as const,
    error: failure,
  }));

  render(
    <GlobalShell
      failureDialogPort={{ present }}
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({ openProject })}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Abrir projeto" }));

  expect(present).toHaveBeenCalledWith({
    context: "projectOpening",
    error: failure,
  });
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Abrir projeto" }),
  ).toBeEnabled();
  expect(screen.queryByText(/(?:[A-Z]:\\|\\\\)/i)).not.toBeInTheDocument();
});

test("loads and renders recent Projects by name", async () => {
  const listRecentProjects = vi.fn(async () => [
    { id: "recent-ana", name: "Álbum da Ana", lastOpenedAtMs: null, favorite: false },
    { id: "recent-bia", name: "Álbum da Bia", lastOpenedAtMs: null, favorite: false },
  ]);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({ listRecentProjects })}
    />,
  );

  expect(
    await screen.findByRole("button", { name: "Álbum da Ana" }),
  ).toBeEnabled();
  expect(
    screen.getByRole("button", { name: "Álbum da Bia" }),
  ).toBeEnabled();
  expect(screen.getByRole("list", { name: "Projetos recentes" })).toBeInTheDocument();
  expect(screen.queryByText("Projeto MyAlbuns")).not.toBeInTheDocument();
  expect(screen.queryByText("Aberto recentemente")).not.toBeInTheDocument();
  expect(screen.queryByText(/Última abertura/)).not.toBeInTheDocument();
  expect(listRecentProjects).toHaveBeenCalledOnce();
});

test("shows the full opening time on its own line and no filler for older records", async () => {
  const dates = welcomePreviewRecentProjects(new URLSearchParams("recents=dates"));
  const { container } = render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      recentProjectsNow={welcomeDatesNow}
      projectPort={createProjectPort({ listRecentProjects: async () => dates })}
    />,
  );
  expect(await screen.findByRole("button", { name: dates[0].name })).toBeEnabled();
  expect(screen.getByText("Aberto hoje às 14:30").tagName).toBe("TIME");
  expect(screen.getByText("Aberto ontem às 09:15").tagName).toBe("TIME");
  expect(screen.getByText("Aberto em 18/09/2026 às 09:15").tagName).toBe("TIME");
  expect(screen.getByRole("button", { name: dates[0].name }))
    .toHaveAttribute("aria-description", "Última abertura: Hoje às 14:30");
  expect(container.querySelectorAll("time")).toHaveLength(3);
  expect(screen.getByRole("button", { name: dates[3].name }).querySelector("time")).toBeNull();
  expect(screen.getByRole("button", { name: dates[3].name }))
    .not.toHaveAttribute("aria-description");
  expect(container.querySelector(".global-project-date-tooltip")).toBeNull();
});

test("opens the full name only over a truncated name and closes it over the preview", async () => {
  const user = userEvent.setup();
  const projects = welcomePreviewRecentProjects(new URLSearchParams("recents=long-names"));
  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      recentProjectsNow={welcomeDatesNow}
      projectPort={createProjectPort({ listRecentProjects: async () => projects })}
    />,
  );
  const first = await screen.findByRole("button", { name: projects[0].name });
  const thumbnail = first.querySelector(".global-project-thumbnail")!;
  const name = first.querySelector("strong")!;
  Object.defineProperties(name, {
    clientWidth: { configurable: true, value: 40 },
    scrollWidth: { configurable: true, value: 240 },
  });
  fireEvent.pointerMove(document.body, { pointerType: "mouse" });

  await user.hover(thumbnail);
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  await user.hover(name);
  expect(await screen.findByRole("tooltip")).toHaveClass("global-project-name-tooltip");
  expect(screen.getByRole("tooltip")).toHaveTextContent(projects[0].name);
  await user.hover(thumbnail);
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
});

test("dismisses a truncated name tooltip with Escape", async () => {
  const user = userEvent.setup();
  const projects = welcomePreviewRecentProjects(new URLSearchParams("recents=long-names"));
  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      recentProjectsNow={welcomeDatesNow}
      projectPort={createProjectPort({ listRecentProjects: async () => projects })}
    />,
  );
  const first = await screen.findByRole("button", { name: projects[0].name });
  const name = first.querySelector("strong")!;
  Object.defineProperties(name, {
    clientWidth: { configurable: true, value: 40 },
    scrollWidth: { configurable: true, value: 240 },
  });
  fireEvent.pointerMove(document.body, { pointerType: "mouse" });
  await user.hover(name);
  expect(await screen.findByRole("tooltip")).toHaveClass("global-project-name-tooltip");
  expect(screen.getByRole("tooltip").parentElement).toBe(document.body);
  await user.keyboard("{Escape}");
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
  await user.unhover(name);
  await user.hover(name);
  expect(await screen.findByRole("tooltip")).toHaveClass("global-project-name-tooltip");
  fireEvent.scroll(window);
  await waitFor(() => expect(screen.queryByRole("tooltip")).not.toBeInTheDocument());
});

test("shows a real first Sheet and keeps an unavailable card usable", async () => {
  const firstRecentProjectSheet = vi.fn(async (id: string) => id === "recent-ana"
    ? {
        sheet: {
          sheetId: "first", number: 1, activeSides: "both" as const,
          widthUm: 600_000, heightUm: 300_000,
          base: { rgb: "#FFFFFF", drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 } },
          backgrounds: [{ kind: "color" as const, rgb: "#FFFFFF", drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 } }],
          frames: [], overlays: [],
        },
        mediaPreviewUrls: {},
      }
    : null);
  const openRecentProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const { container } = render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({
        listRecentProjects: async () => [
          { id: "recent-ana", name: "Álbum da Ana", lastOpenedAtMs: null, favorite: false },
          { id: "recent-bia", name: "Álbum da Bia", lastOpenedAtMs: null, favorite: false },
        ],
        firstRecentProjectSheet,
        openRecentProject,
      })}
    />,
  );
  expect(await screen.findByRole("img", { name: "Prévia da lâmina 01", hidden: true })).toBeInTheDocument();
  expect(container.querySelector('.global-project-thumbnail[data-preview-state="ready"] .sheet-preview')).toBeInTheDocument();
  expect(container.querySelector('.global-project-thumbnail[data-preview-state="ready"] [data-preview-background-color="#FFFFFF"]')).toBeInTheDocument();
  expect(container.querySelector('.global-project-thumbnail[data-preview-state="ready"] .global-project-album')).not.toBeInTheDocument();
  await waitFor(() => expect(container.querySelector('.global-project-thumbnail[data-preview-state="unavailable"] .global-project-album')).toBeInTheDocument());
  expect(container.querySelector('.global-project-thumbnail[data-preview-state="unavailable"] .sheet-preview')).not.toBeInTheDocument();
  await userEvent.setup().click(screen.getByRole("button", { name: "Álbum da Bia" }));
  expect(openRecentProject).toHaveBeenCalledWith("recent-bia");
  expect(firstRecentProjectSheet).toHaveBeenCalledWith("recent-ana");
  expect(firstRecentProjectSheet).toHaveBeenCalledWith("recent-bia");
});

test("keeps a pending first Sheet represented without blocking its card", async () => {
  const firstRecentProjectSheet = vi.fn(() => new Promise<never>(() => undefined));
  const { container } = render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({
        listRecentProjects: async () => [{ id: "recent-ana", name: "Álbum da Ana", lastOpenedAtMs: null, favorite: false }],
        firstRecentProjectSheet,
      })}
    />,
  );
  expect(await screen.findByRole("button", { name: "Álbum da Ana" })).toBeEnabled();
  await waitFor(() => expect(firstRecentProjectSheet).toHaveBeenCalledWith("recent-ana"));
  expect(container.querySelector('.global-project-thumbnail[data-preview-state="loading"] .global-project-album')).toBeInTheDocument();
  expect(container.querySelector('.global-project-thumbnail[data-preview-state="loading"] .sheet-preview')).not.toBeInTheDocument();
});

test("reopens a recent Project using only its opaque id", async () => {
  const user = userEvent.setup();
  const opening = deferred<OpenProjectOutcome>();
  const openProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const openRecentProject = vi.fn(() => opening.promise);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({
        listRecentProjects: async () => [
          { id: "recent-ana", name: "Álbum da Ana", lastOpenedAtMs: null, favorite: false },
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

test("moves favorites between groups without opening and restores focus after the last favorite is removed", async () => {
  const user = userEvent.setup();
  const projects = [
    { id: "newer", name: "Mais recente", lastOpenedAtMs: 200, favorite: false },
    { id: "older", name: "Preferido", lastOpenedAtMs: 100, favorite: false },
  ];
  const openRecentProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const setRecentProjectFavorite = vi.fn(async (id: string, favorite: boolean) => ({
    status: "saved" as const,
    projects: projects.map((project) => project.id === id ? { ...project, favorite } : project),
  }));
  render(<GlobalShell graphicsDiagnostic={supportedGraphics} projectPort={createProjectPort({
    listRecentProjects: async () => projects,
    setRecentProjectFavorite,
    openRecentProject,
  })} />);
  await screen.findByRole("button", { name: "Preferido" });
  const olderCard = screen.getByRole("button", { name: "Preferido" }).closest("li")!;
  const olderStar = olderCard.querySelector<HTMLButtonElement>(".global-project-favorite")!;
  olderStar.focus();
  await user.keyboard("{Enter}");
  expect(setRecentProjectFavorite).toHaveBeenCalledWith("older", true);
  expect(openRecentProject).not.toHaveBeenCalled();
  expect(screen.getByRole("list", { name: "Favoritos" })).toHaveTextContent("Preferido");
  expect(screen.getByRole("list", { name: "Projetos recentes" })).toHaveTextContent("Mais recente");
  const unmark = screen.getByRole("button", { name: "Remover dos favoritos" });
  expect(unmark).toHaveAttribute("aria-pressed", "true");
  expect(unmark).toHaveFocus();
  await user.keyboard(" ");
  expect(setRecentProjectFavorite).toHaveBeenCalledWith("older", false);
  expect(screen.queryByRole("heading", { name: "Favoritos" })).not.toBeInTheDocument();
  expect(screen.getAllByRole("button", { name: "Adicionar aos favoritos" })).toHaveLength(2);
  expect(screen.getByRole("button", { name: "Preferido" }).closest("li")?.querySelector(".global-project-favorite")).toHaveFocus();
});

test("keeps the saved favorite when the update fails and reports the operational error", async () => {
  const present = vi.fn(async () => undefined);
  const openRecentProject = vi.fn(async () => ({ status: "cancelled" as const }));
  render(<GlobalShell graphicsDiagnostic={supportedGraphics}
    failureDialogPort={{ present }} projectPort={createProjectPort({
      listRecentProjects: async () => [{ id: "one", name: "Um projeto", lastOpenedAtMs: 100, favorite: true }],
      setRecentProjectFavorite: async () => ({ status: "failed", error: {
        code: "recent_project_favorite_unavailable", message: "Falha", action: "Tente novamente.",
      } }),
      openRecentProject,
    })} />);
  await userEvent.setup().click(await screen.findByRole("button", { name: "Remover dos favoritos" }));
  expect(screen.getByRole("list", { name: "Favoritos" })).toHaveTextContent("Um projeto");
  expect(present).toHaveBeenCalledWith({ context: "favoriteUpdate", error: {
    code: "recent_project_favorite_unavailable", message: "Falha", action: "Tente novamente.",
  } });
  expect(screen.getByRole("button", { name: "Remover dos favoritos" })).toHaveFocus();
  expect(openRecentProject).not.toHaveBeenCalled();
});

test("moves focus to a remaining recent action when retention removes the unfavorited card", async () => {
  const newer = { id: "newer", name: "Mais recente", lastOpenedAtMs: 200, favorite: false };
  const older = { id: "older", name: "Favorito antigo", lastOpenedAtMs: 100, favorite: true };
  render(<GlobalShell graphicsDiagnostic={supportedGraphics} projectPort={createProjectPort({
    listRecentProjects: async () => [newer, older],
    setRecentProjectFavorite: async () => ({ status: "saved", projects: [newer] }),
  })} />);
  await userEvent.setup().click(await screen.findByRole("button", { name: "Remover dos favoritos" }));
  expect(screen.queryByRole("button", { name: "Favorito antigo" })).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Mais recente" }).closest("li")
    ?.querySelector(".global-project-favorite")).toHaveFocus();
});

test("disables every favorite action while a favorite write is pending", async () => {
  const pending = deferred<{ status: "saved"; projects: readonly {
    id: string; name: string; lastOpenedAtMs: number; favorite: boolean;
  }[] }>();
  const projects = [
    { id: "one", name: "Primeiro", lastOpenedAtMs: 200, favorite: false },
    { id: "two", name: "Segundo", lastOpenedAtMs: 100, favorite: false },
  ];
  const setRecentProjectFavorite = vi.fn(() => pending.promise);
  render(<GlobalShell graphicsDiagnostic={supportedGraphics} projectPort={createProjectPort({
    listRecentProjects: async () => projects,
    setRecentProjectFavorite,
  })} />);
  await screen.findByRole("button", { name: "Primeiro" });
  const stars = screen.getAllByRole("button", { name: "Adicionar aos favoritos" });
  await userEvent.setup().click(stars[0]);
  expect(stars[0]).toBeDisabled();
  expect(stars[1]).toBeDisabled();
  expect(screen.getByRole("button", { name: "Segundo" })).toBeEnabled();
  expect(setRecentProjectFavorite).toHaveBeenCalledOnce();
  await act(async () => pending.resolve({ status: "saved", projects: [
    { ...projects[0], favorite: true }, projects[1],
  ] }));
  expect(screen.getByRole("button", { name: "Remover dos favoritos" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "Adicionar aos favoritos" })).toBeEnabled();
});

test("shows the startup failure from a direct Windows opening", async () => {
  const present = vi.fn(async () => undefined);
  const startupOpenFailure = vi.fn(async () => ({
    code: "invalid_project",
    message: "O arquivo selecionado não é um projeto válido.",
    action: "Escolha outro arquivo .myalbuns.",
  }));

  render(
    <GlobalShell
      failureDialogPort={{ present }}
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({
        startupOpenFailure,
      })}
    />,
  );

  await waitFor(() => {
    expect(present).toHaveBeenCalledWith({
      context: "projectOpening",
      error: {
        code: "invalid_project",
        message: "O arquivo selecionado não é um projeto válido.",
        action: "Escolha outro arquivo .myalbuns.",
      },
    });
  });
  expect(startupOpenFailure).toHaveBeenCalledOnce();
});

test("does not overwrite a newer opening attempt with a late startup failure", async () => {
  const user = userEvent.setup();
  const startupFailure = deferred<OpenProjectFailure | null>();
  const present = vi.fn(async () => undefined);

  render(
    <GlobalShell
      failureDialogPort={{ present }}
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({
        openProject: async () => ({ status: "cancelled" }),
        startupOpenFailure: () => startupFailure.promise,
      })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Abrir projeto" }));
  await act(async () => {
    startupFailure.resolve({
      code: "stale_startup_failure",
      message: "Esta falha pertence à tentativa inicial.",
    });
  });

  expect(present).not.toHaveBeenCalled();
});

test("reacts to terminal outcomes forwarded after Global mounted and releases the listener", async () => {
  const present = vi.fn(async () => undefined);
  let activationListener:
    | ((outcome: OpenProjectOutcome) => void)
    | undefined;
  const unlisten = vi.fn();
  const onActivationTerminal = vi.fn(
    async (listener: (outcome: OpenProjectOutcome) => void) => {
      activationListener = listener;
      return unlisten;
    },
  );

  const view = render(
    <GlobalShell
      failureDialogPort={{ present }}
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({ onActivationTerminal })}
    />,
  );

  await waitFor(() => expect(onActivationTerminal).toHaveBeenCalledOnce());

  act(() => {
    activationListener?.({
      status: "failed",
      error: {
        code: "project_in_use",
        message: "Este projeto está aberto por outra instância.",
        action: "Focalize a instância proprietária.",
      },
    });
  });
  await waitFor(() =>
    expect(present).toHaveBeenCalledWith({
      context: "projectOpening",
      error: {
        code: "project_in_use",
        message: "Este projeto está aberto por outra instância.",
        action: "Focalize a instância proprietária.",
      },
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();

  act(() => activationListener?.({ status: "cancelled" }));
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(present).toHaveBeenCalledOnce();

  view.unmount();
  await waitFor(() => expect(unlisten).toHaveBeenCalledOnce());
});

test("does not overwrite a forwarded terminal with a late graphics-gate outcome", async () => {
  const graphicsGate = deferred<OpenProjectOutcome | null>();
  const present = vi.fn(async () => undefined);
  let activationListener:
    | ((outcome: OpenProjectOutcome) => void)
    | undefined;

  render(
    <GlobalShell
      failureDialogPort={{ present }}
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({
        completeGraphicsGate: () => graphicsGate.promise,
        onActivationTerminal: async (listener) => {
          activationListener = listener;
          return () => undefined;
        },
      })}
    />,
  );
  await waitFor(() => expect(activationListener).toBeTypeOf("function"));

  act(() => {
    activationListener?.({
      status: "failed",
      error: {
        code: "forwarded_failure",
        message: "A abertura encaminhada falhou.",
      },
    });
  });
  await act(async () => {
    graphicsGate.resolve({
      status: "failed",
      error: {
        code: "stale_graphics_gate_failure",
        message: "Esta falha inicial já foi substituída.",
      },
    });
  });

  await waitFor(() => expect(present).toHaveBeenCalledOnce());
  expect(present).toHaveBeenCalledWith({
    context: "projectOpening",
    error: {
      code: "forwarded_failure",
      message: "A abertura encaminhada falhou.",
    },
  });
});


test("editor entry opens directly in New Project and repeated activation preserves its draft", async () => {
  let activate!: () => void;
  const release = vi.fn();
  const requests = vi.fn(async (listener: () => void) => { activate = listener; return release; });
  const { unmount } = render(<GlobalShell initialSurface="newProject" onNewProjectRequest={requests}
    graphicsDiagnostic={supportedGraphics} projectPort={createProjectPort()} />);
  expect(screen.queryByRole("heading", { name: "Projetos recentes" })).not.toBeInTheDocument();
  const count = screen.getByRole("textbox", { name: "Lâminas" });
  fireEvent.change(count, { target: { value: "23" } });
  await waitFor(() => expect(requests).toHaveBeenCalledOnce());
  act(() => activate());
  expect(count).toHaveValue("23");
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.getByRole("status", { name: "Nenhum projeto recente" })).toBeInTheDocument();
  act(() => activate());
  expect(screen.getByRole("textbox", { name: "Lâminas" })).toBeVisible();
  unmount();
  expect(release).toHaveBeenCalledOnce();
});
