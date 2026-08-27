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
    onActivationTerminal: async () => () => undefined,
    completeGraphicsGate: async () => null,
    openProject: async () => ({ status: "cancelled" }),
    saveExternalCopyAs: async () => ({ status: "cancelled" }),
    listRecentProjects: async () => [],
    openRecentProject: async () => ({ status: "cancelled" }),
    startupOpenFailure: async () => null,
    ...overrides,
  };
}

test("shows the global welcome surface without a Project workspace", () => {
  const projectPort = createProjectPort({ openProject: vi.fn() });

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={projectPort}
    />,
  );

  expect(
    screen.getByRole("heading", { name: "Projetos recentes" }),
  ).toBeInTheDocument();
  const newProjectButton = screen.getByRole("button", {
    name: "Novo Projeto",
  });
  const openProjectButton = screen.getByRole("button", {
    name: "Abrir Projeto",
  });
  expect(newProjectButton).toBeEnabled();
  expect(newProjectButton).toHaveAttribute("aria-keyshortcuts", "Control+N");
  expect(openProjectButton).toBeEnabled();
  expect(openProjectButton).toHaveAttribute("aria-keyshortcuts", "Control+O");
  const batchExportPlaceholder = screen.getByRole("button", {
    name: "Exportação em lote",
  });
  expect(batchExportPlaceholder).toBeDisabled();
  expect(batchExportPlaceholder).toHaveAttribute(
    "data-placeholder-feature",
    "batch-export",
  );
  expect(screen.queryByTestId("album-canvas")).not.toBeInTheDocument();
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
  ).toHaveTextContent("Novo Projeto");

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

  expect(screen.getByRole("button", { name: "Novo Projeto" })).toHaveFocus();
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
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort()}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Novo Projeto" }));

  const windowTitlebar = screen.getByRole("banner", {
    name: "Barra da janela",
  });
  expect(screen.getAllByText("Novo Projeto")).toHaveLength(1);
  expect(windowTitlebar).toContainElement(screen.getByText("Novo Projeto"));
  expect(
    screen.queryByRole("heading", { level: 1, name: "Novo Projeto" }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("heading", { name: "Projetos recentes" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getAllByRole("banner", { name: "Barra da janela" }),
  ).toHaveLength(1);

  await user.click(screen.getByRole("button", { name: "Cancelar" }));

  expect(
    screen.getByRole("heading", { name: "Projetos recentes" }),
  ).toHaveClass("ui-section-eyebrow");
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

  await user.click(screen.getByRole("button", { name: "Novo Projeto" }));
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
  const present = vi.fn(async () => undefined);
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
      failureDialogPort={{ present }}
      graphicsDiagnostic={supportedGraphics}
      newProjectPort={createNewProjectPortStub()}
      projectPort={createProjectPort({ openProject })}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

  expect(present).toHaveBeenCalledWith({
    context: "projectOpening",
    error: failure,
  });
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Abrir Projeto" }),
  ).toBeEnabled();
  expect(screen.queryByText(/\.(?:myalbuns)|\\|:\//i)).not.toBeInTheDocument();
});

test("offers Salvar cópia como for a validated read-only external copy", async () => {
  const user = userEvent.setup();
  const openProject = vi.fn(async () => ({
    status: "externalCopyNotWritable" as const,
  }));
  const saveExternalCopyAs = vi.fn(async () => ({
    status: "cancelled" as const,
  }));

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({
        openProject,
        saveExternalCopyAs,
      })}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

  expect(
    await screen.findByRole("heading", {
      name: "Cópia externa somente leitura",
    }),
  ).toBeInTheDocument();
  const saveCopy = screen.getByRole("button", {
    name: "Salvar cópia como…",
  });
  await user.click(saveCopy);

  expect(saveExternalCopyAs).toHaveBeenCalledOnce();
  expect(
    screen.queryByRole("heading", {
      name: "Cópia externa somente leitura",
    }),
  ).not.toBeInTheDocument();
  expect(screen.queryByText(/\.(?:myalbuns)|\\|:\//i)).not.toBeInTheDocument();
});

test("keeps resolving queued read-only external copies one at a time", async () => {
  const user = userEvent.setup();
  const saveExternalCopyAs = vi
    .fn<() => Promise<OpenProjectOutcome>>()
    .mockResolvedValueOnce({ status: "externalCopyNotWritable" })
    .mockResolvedValueOnce({ status: "cancelled" });

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({
        openProject: async () => ({
          status: "externalCopyNotWritable",
        }),
        saveExternalCopyAs,
      })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));
  const saveCopy = await screen.findByRole("button", {
    name: "Salvar cópia como…",
  });
  await user.click(saveCopy);

  expect(saveExternalCopyAs).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("heading", {
      name: "Cópia externa somente leitura",
    }),
  ).toBeInTheDocument();

  await user.click(saveCopy);
  expect(saveExternalCopyAs).toHaveBeenCalledTimes(2);
  expect(
    screen.queryByRole("heading", {
      name: "Cópia externa somente leitura",
    }),
  ).not.toBeInTheDocument();
});

test("offers Salvar cópia como after a direct Windows opening", async () => {
  const completeGraphicsGate = vi.fn(async () => ({
    status: "externalCopyNotWritable" as const,
  }));

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({ completeGraphicsGate })}
    />,
  );

  expect(
    await screen.findByRole("heading", {
      name: "Cópia externa somente leitura",
    }),
  ).toBeInTheDocument();
  expect(completeGraphicsGate).toHaveBeenCalledWith(true);
});

test.each([
  ["cancelled", { status: "cancelled" as const }],
  [
    "failed",
    {
      status: "failed" as const,
      error: {
        code: "dialog_unavailable",
        message: "Não foi possível concluir o diálogo de abertura.",
        action: "Tente novamente.",
      },
    },
  ],
])("keeps the pending external copy visible after a later %s opening", async (_, laterOutcome) => {
  const user = userEvent.setup();
  const openProject = vi
    .fn<() => Promise<OpenProjectOutcome>>()
    .mockResolvedValueOnce({ status: "externalCopyNotWritable" })
    .mockResolvedValueOnce(laterOutcome);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({ openProject })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));
  const pendingResolution = await screen.findByRole("heading", {
    name: "Cópia externa somente leitura",
  });

  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));

  expect(pendingResolution).toBeInTheDocument();
  expect(openProject).toHaveBeenCalledTimes(2);
});

test("loads and renders recent Projects by name", async () => {
  const listRecentProjects = vi.fn(async () => [
    { id: "recent-ana", name: "Álbum da Ana" },
    { id: "recent-bia", name: "Álbum da Bia" },
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
  expect(screen.getByRole("list", { name: "Projetos recentes" })).toHaveAttribute(
    "data-placeholder-feature",
    "recent-project-visual-metadata",
  );
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
      newProjectPort={createNewProjectPortStub()}
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
  const present = vi.fn(async () => undefined);
  const startupOpenFailure = vi.fn(async () => ({
    code: "invalid_project",
    message: "O arquivo selecionado não é um Projeto válido.",
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
        message: "O arquivo selecionado não é um Projeto válido.",
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

  await user.click(screen.getByRole("button", { name: "Abrir Projeto" }));
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
        message: "Este Projeto está aberto por outra instância.",
        action: "Focalize a instância proprietária.",
      },
    });
  });
  await waitFor(() =>
    expect(present).toHaveBeenCalledWith({
      context: "projectOpening",
      error: {
        code: "project_in_use",
        message: "Este Projeto está aberto por outra instância.",
        action: "Focalize a instância proprietária.",
      },
    }),
  );
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();

  act(() => {
    activationListener?.({ status: "externalCopyNotWritable" });
  });
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(present).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("heading", {
      name: "Cópia externa somente leitura",
    }),
  ).toBeInTheDocument();

  view.unmount();
  await waitFor(() => expect(unlisten).toHaveBeenCalledOnce());
});

test("does not overwrite a forwarded terminal with a late graphics-gate outcome", async () => {
  const graphicsGate = deferred<OpenProjectOutcome | null>();
  let activationListener:
    | ((outcome: OpenProjectOutcome) => void)
    | undefined;

  render(
    <GlobalShell
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
    activationListener?.({ status: "externalCopyNotWritable" });
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

  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", {
      name: "Cópia externa somente leitura",
    }),
  ).toBeInTheDocument();
});
