import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type {
  GlobalProjectPort,
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
    validateProjectConfiguration: async () => ({ status: "valid" }),
    createProject: async () => ({ status: "cancelled" }),
    chooseProvisionalDecorative: async () => ({ status: "cancelled" }),
    releaseProvisionalDecorative: async () => undefined,
    clearProvisionalDecoratives: async () => undefined,
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
  expect(screen.queryByTestId("album-canvas")).not.toBeInTheDocument();
});

test("blocks Project hosts at the global graphics boundary when hardware WebGL2 is unavailable", async () => {
  const completeGraphicsGate = vi.fn(async () => null);
  const openProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const createProject = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <GlobalShell
      graphicsDiagnostic={unavailableGraphics}
      projectPort={createProjectPort({
        completeGraphicsGate,
        createProject,
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

test("opens and cancels the creation assistant without starting Project creation", async () => {
  const user = userEvent.setup();
  const createProject = vi.fn(async () => ({ status: "cancelled" as const }));
  const clearProvisionalDecoratives = vi.fn(async () => undefined);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({
        createProject,
        clearProvisionalDecoratives,
      })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Novo Projeto" }));
  expect(screen.getByRole("dialog")).toHaveAccessibleName("Dimensões");

  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(createProject).not.toHaveBeenCalled();
  expect(clearProvisionalDecoratives).toHaveBeenCalledOnce();
});

test("connects provisional selection and owns final registry cleanup in Global", async () => {
  const user = userEvent.setup();
  const selection = {
    selectionId: "selection-background",
    displayName: "Textura.jpg",
    previewUrl: "myalbuns-preview://localhost/selection-background",
  };
  const chooseProvisionalDecorative = vi.fn(async () => ({
    status: "selected" as const,
    selection,
  }));
  const releaseProvisionalDecorative = vi.fn(async () => undefined);
  const clearProvisionalDecoratives = vi.fn(async () => undefined);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({
        chooseProvisionalDecorative,
        releaseProvisionalDecorative,
        clearProvisionalDecoratives,
      })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Novo Projeto" }));
  await user.click(screen.getByRole("button", { name: /Pr.*ximo/ }));
  await user.click(
    await screen.findByRole("button", {
      name: "Escolher imagem de Background",
    }),
  );

  expect(chooseProvisionalDecorative).toHaveBeenCalledOnce();
  expect(await screen.findByText(selection.displayName)).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Cancelar" }));

  expect(releaseProvisionalDecorative).not.toHaveBeenCalled();
  expect(clearProvisionalDecoratives).toHaveBeenCalledOnce();
});

test("forwards the completed dimensions configuration to creation", async () => {
  const user = userEvent.setup();
  const createProject = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({ createProject })}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Novo Projeto" }));
  await user.click(screen.getByRole("button", { name: "Próximo" }));
  await user.click(await screen.findByRole("button", { name: "Criar" }));

  expect(createProject).toHaveBeenCalledWith({
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
      lastSheet: "double",
    },
    visualDefaults: {
      background: {
        scope: "bothSides",
        both: { kind: "color", rgb: "#FFFFFF" },
      },
      overlay: { scope: "bothSides", both: null },
      frameBorder: { kind: "none" },
    },
  });
});

test("starts one opening attempt and reports that it is in progress", async () => {
  const user = userEvent.setup();
  const opening = deferred<OpenProjectOutcome>();
  const openProject = vi.fn(() => opening.promise);

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({ openProject })}
    />,
  );

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

  render(
    <GlobalShell
      graphicsDiagnostic={supportedGraphics}
      projectPort={createProjectPort({ openProject })}
    />,
  );
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
      graphicsDiagnostic={supportedGraphics}
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
      graphicsDiagnostic={supportedGraphics}
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
      graphicsDiagnostic={supportedGraphics}
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
