import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, expect, test, vi } from "vitest";

import type {
  ExportOutcome,
  ExportPort,
  ExportProgressEvent,
  MediaPreviewDemand,
  ProjectCloseResolution,
  ProjectSessionPort,
  ProjectWindowPort,
} from "../application/projectPorts";
import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "../application/projectDialogPort";
import {
  ProjectCloseError,
  SaveProjectError,
} from "../application/projectPorts";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import {
  createEmptyProjection,
  createTwoSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import type {
  AlbumCanvasMode,
  CanvasMetrics,
  CanvasTechnicalGuides,
  PhotoTransformDelta,
  PhotoTransformPreview,
} from "./AlbumCanvas";
import type { ContinuousCanvasLayout } from "./canvasGeometry";
import { ProjectWorkspace as ProjectWorkspaceView } from "./ProjectWorkspace";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

const canvasHarness = vi.hoisted(() => ({
  props: null as null | {
    mode: AlbumCanvasMode;
    continuousCanvasLayout: ContinuousCanvasLayout;
    mediaPreviewUrls?: Readonly<Record<string, string>>;
    technicalGuides?: CanvasTechnicalGuides;
    onMediaDemandChange?(demand: MediaPreviewDemand): void;
    onCanvasMetricsChange?(metrics: CanvasMetrics): void;
    onCenteredSheetChange?(sheetId: string): void;
    onEditSheet?(sheetId: string): void;
    onTransformPreview?(
      preview: PhotoTransformPreview | null,
    ): void;
    onTransformCommit(
      delta: PhotoTransformDelta,
    ): Promise<boolean>;
    onGraphicsUnavailable?(diagnostic: GraphicsDiagnostic): void;
  },
}));

interface ObservedViewport {
  callback: IntersectionObserverCallback;
  options: IntersectionObserverInit;
  targets: Set<Element>;
}

const observedViewports: ObservedViewport[] = [];

function emitPanelIntersections(
  rootMargin: string,
  intersections: Readonly<Record<string, boolean>>,
) {
  const viewport = observedViewports.find(
    ({ options }) => options.rootMargin === rootMargin,
  );
  if (!viewport) throw new Error(`Observer ${rootMargin} ausente.`);
  const entries = Object.entries(intersections).map(
    ([mediaId, isIntersecting]) => ({
      target: document.querySelector(`[data-media-id="${mediaId}"]`)!,
      isIntersecting,
    }),
  );
  viewport.callback(
    entries as IntersectionObserverEntry[],
    {} as IntersectionObserver,
  );
}

vi.mock("./AlbumCanvas", () => ({
  AlbumCanvas: (props: typeof canvasHarness.props) => {
    canvasHarness.props = props;
    return <div data-testid="album-canvas" />;
  },
}));

const projection = representativeProjection;
const twoSheetProjection = createTwoSheetProjection();
const decorativePreviewUrl =
  "asset://localhost/cache/decorative-overlay.png";
const decorativeProjection: EditorProjection = {
  ...projection,
  state: {
    ...projection.state,
    album: {
      ...projection.state.album,
      visualDefaults: {
        ...projection.state.album.visualDefaults,
        overlay: {
          scope: "bothSides",
          both: { kind: "media", mediaId: "decorative-overlay" },
        },
      },
      media: [
        ...projection.state.album.media,
        {
          id: "decorative-overlay",
          kind: "decorative",
          name: "Overlay translúcido.png",
          sourceWidthPx: 2_400,
          sourceHeightPx: 1_800,
          palette: ["#17344a", "#88b7c5", "#d4a15e"],
        },
      ],
    },
  },
  composition: {
    ...projection.composition,
    sheets: projection.composition.sheets.map((sheet) => ({
      ...sheet,
      overlays: [
        {
          mediaId: "decorative-overlay",
          name: "Overlay translúcido.png",
          drawRect: {
            x: 0,
            y: 0,
            width: sheet.widthUm,
            height: sheet.heightUm,
          },
        },
      ],
    })),
  },
  mediaUsage: [
    ...projection.mediaUsage,
    { mediaId: "decorative-overlay", count: 1 },
  ],
};

function deferredProjection() {
  let resolve!: (value: EditorProjection) => void;
  const promise = new Promise<EditorProjection>((resolver) => {
    resolve = resolver;
  });
  return { promise, resolve };
}

const exportPort: ExportPort = {
  startSheet: () => ({
    completion: Promise.resolve({
      status: "completed",
      result: {
        widthPx: 600,
        heightPx: 300,
      },
    }),
    cancel: async () => "not_found",
  }),
};

const inertProjectWindowPort: ProjectWindowPort = {
  onCloseRequested: async () => () => undefined,
  requestClose: async () => ({ kind: "closed" }),
  resolveClose: async () => ({ kind: "closed" }),
};

const inertProjectDialogPort: ProjectDialogPort = {
  dismiss: async () => undefined,
  onAction: async () => () => undefined,
  present: async () => undefined,
};

function projectDialogHarness() {
  const listeners = new Set<(action: ProjectDialogAction) => void>();
  const dismiss = vi.fn(async () => undefined);
  const present = vi.fn(async () => undefined);
  const onAction = vi.fn<ProjectDialogPort["onAction"]>(async (nextListener) => {
    listeners.add(nextListener);
    return () => {
      listeners.delete(nextListener);
    };
  });
  return {
    dismiss,
    emit: (action: ProjectDialogAction) => {
      for (const listener of listeners) listener(action);
    },
    onAction,
    port: { dismiss, onAction, present } satisfies ProjectDialogPort,
    present,
  };
}

function projectWindowHarness() {
  let closeRequested: (() => void) | null = null;
  const port: ProjectWindowPort = {
    onCloseRequested: async (listener) => {
      closeRequested = listener;
      return () => {
        closeRequested = null;
      };
    },
    requestClose: vi.fn(async () => ({
      kind: "confirmationRequired" as const,
    })),
    resolveClose: vi.fn(async () => ({ kind: "closed" as const })),
  };
  return {
    dialog: projectDialogHarness(),
    port,
    emitCloseRequested: () => closeRequested?.(),
  };
}

function projectSessionPortWithApply(
  apply: ProjectSessionPort["apply"],
): ProjectSessionPort {
  return {
    load: async () => projection,
    validateAlbumInformation: async () => ({
      errors: [],
      impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
    }),
    apply,
    undo: async () => projection,
    redo: async () => projection,
    save: async () => {
      throw new Error("Salvamento não configurado neste teste.");
    },
  };
}

type TestProjectWorkspaceProps = Omit<
  ComponentProps<typeof ProjectWorkspaceView>,
  | "runProjectMutation"
  | "projectDialogPort"
  | "projectWindowPort"
  | "validateAlbumInformation"
> & {
  projectDialogPort?: ProjectDialogPort;
  projectSessionPort: ProjectSessionPort;
  projectWindowPort?: ProjectWindowPort;
};

function ProjectWorkspace({
  projectDialogPort = inertProjectDialogPort,
  projectSessionPort,
  projectWindowPort = inertProjectWindowPort,
  projection,
  ...props
}: TestProjectWorkspaceProps) {
  const runProjectMutation = useProjectMutationRunner(
    projection.state.projectId,
    projectSessionPort,
  );
  return (
    <ProjectWorkspaceView
      {...props}
      projectDialogPort={projectDialogPort}
      projection={projection}
      projectWindowPort={projectWindowPort}
      runProjectMutation={runProjectMutation}
      validateAlbumInformation={projectSessionPort.validateAlbumInformation}
    />
  );
}

function getApplicationCommand(
  menuName:
    | "Arquivo"
    | "Editar"
    | "Lâmina"
    | "Exibir"
    | "Ferramentas"
    | "Ajuda",
  commandName: string,
) {
  const menu = screen.getByRole("menuitem", { name: menuName });
  if (menu.getAttribute("aria-expanded") !== "true") {
    fireEvent.click(menu);
  }
  return screen.getByRole("menuitem", { name: commandName });
}

beforeEach(() => {
  canvasHarness.props = null;
  localStorage.clear();
  observedViewports.length = 0;
  vi.stubGlobal(
    "IntersectionObserver",
    class {
      readonly viewport: ObservedViewport;

      constructor(
        callback: IntersectionObserverCallback,
        options: IntersectionObserverInit = {},
      ) {
        this.viewport = { callback, options, targets: new Set() };
        observedViewports.push(this.viewport);
      }

      observe(target: Element) {
        this.viewport.targets.add(target);
      }

      unobserve(target: Element) {
        this.viewport.targets.delete(target);
      }

      disconnect() {
        this.viewport.targets.clear();
      }

      takeRecords() {
        return [];
      }
    },
  );
  useEditorView.setState({
    projectId: projection.state.projectId,
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 42 },
  });
});

test("presents the canonical desktop menus and marks unfinished commands", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  for (const menuName of [
    "Arquivo",
    "Editar",
    "Lâmina",
    "Exibir",
    "Ferramentas",
    "Ajuda",
  ]) {
    expect(screen.getByRole("menuitem", { name: menuName })).toBeEnabled();
  }
  expect(screen.queryByRole("button", { name: "Inserir" })).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: "Visualizar" }),
  ).not.toBeInTheDocument();

  expect(getApplicationCommand("Arquivo", "Salvar")).toBeEnabled();
  const newProject = getApplicationCommand("Arquivo", "Novo Projeto…");
  expect(newProject).toBeDisabled();
  expect(newProject).toHaveAttribute(
    "data-placeholder-feature",
    "new-project-from-project-window",
  );

  expect(getApplicationCommand("Editar", "Adicionar Frame")).toBeDisabled();
  expect(getApplicationCommand("Editar", "Copiar")).toBeDisabled();
  expect(
    screen.queryByRole("menuitem", { name: "Copiar Frames" }),
  ).not.toBeInTheDocument();
  expect(
    getApplicationCommand("Editar", "Trocar conteúdo dos Frames"),
  ).toBeDisabled();
  expect(
    getApplicationCommand("Editar", "Salvar disposição como Layout"),
  ).toBeDisabled();
  const arrange = getApplicationCommand("Editar", "Organizar");
  expect(arrange).toHaveAttribute("aria-haspopup", "menu");
  fireEvent.click(arrange);
  expect(
    screen.getByRole("menuitem", { name: "Trazer para frente" }),
  ).toBeDisabled();
  expect(getApplicationCommand("Lâmina", "Adicionar depois")).toBeDisabled();
  expect(getApplicationCommand("Exibir", "Painel de imagens")).toBeDisabled();
  expect(getApplicationCommand("Ferramentas", "Configurações…")).toBeDisabled();
  expect(getApplicationCommand("Ajuda", "Manual do MyAlbuns")).toBeDisabled();
});

test("derives the Canvas technical guides from the canonical document", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(canvasHarness.props?.technicalGuides).toEqual({
    bleedUm: projection.state.document.bleedUm,
    safetyUm: projection.state.document.safetyUm,
  });
});

test("temporarily compacts the image panel during Sheet Edit Mode and restores its normal height", () => {
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );
  const workspace = view.container.querySelector(
    ".workspace-grid",
  ) as HTMLElement;
  const normalHeight = workspace.style.getPropertyValue(
    "--media-panel-height",
  );

  act(() => canvasHarness.props?.onEditSheet?.("sheet-001"));

  expect(canvasHarness.props?.mode).toEqual({
    kind: "sheet-editing",
    sheetId: "sheet-001",
  });
  expect(
    workspace.style.getPropertyValue("--media-panel-height"),
  ).toBe("120px");
  expect(
    screen.getByRole("separator", {
      name: "Redimensionar Painel de imagens",
    }),
  ).toHaveAttribute("aria-disabled", "true");

  const input = document.createElement("input");
  document.body.append(input);
  fireEvent.keyDown(input, { key: "Escape" });

  expect(canvasHarness.props?.mode).toEqual({ kind: "normal" });
  expect(
    workspace.style.getPropertyValue("--media-panel-height"),
  ).toBe(normalHeight);
  input.remove();
});

test("starts the implemented Lâmina export from the Arquivo menu", () => {
  const startSheet = vi.fn<ExportPort["startSheet"]>(() => ({
    completion: Promise.resolve({
      status: "completed",
      result: { widthPx: 600, heightPx: 300 },
    }),
    cancel: async () => "not_found",
  }));
  render(
    <ProjectWorkspace
      exportPort={{ startSheet }}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(getApplicationCommand("Arquivo", "Exportar Lâmina…"));

  expect(startSheet).toHaveBeenCalledWith("sheet-001", expect.any(Function));
});

test("uses contextual empty states when the editor has no materialized content", () => {
  const emptyProjection = createEmptyProjection();
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={emptyProjection}
      projectSessionPort={projectSessionPortWithApply(async () =>
        emptyProjection
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const mediaEmptyState = screen.getByRole("status", {
    name: "Nenhuma Foto importada",
  });

  expect(mediaEmptyState).toHaveClass("ui-empty-state");
  expect(mediaEmptyState).toHaveClass("media-empty-state--catalog");
  expect(
    screen.getByRole("status", { name: "Nenhuma Lâmina na Grade" }),
  ).toHaveClass("ui-empty-state");
});

test("offers the three close choices for a native request and Cancel keeps the Project", async () => {
  const harness = projectWindowHarness();
  const dirtyProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: true,
    },
  };
  harness.port.resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection: dirtyProjection,
  }));

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={dirtyProjection}
      projectSessionPort={projectSessionPortWithApply(async () =>
        dirtyProjection
      )}
      projectDialogPort={harness.dialog.port}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    expect(harness.emitCloseRequested()).toBeUndefined();
    expect(harness.dialog.present).toHaveBeenCalledWith({
      busy: false,
      kind: "projectCloseConfirmation",
    });
  });

  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    screen.getByRole("menuitem", { name: "Editar", hidden: true }),
  ).toBeDisabled();

  act(() => harness.dialog.emit("cancelProjectClose"));

  await waitFor(() => {
    expect(harness.port.resolveClose).toHaveBeenCalledWith("cancel");
    expect(harness.dialog.dismiss).toHaveBeenCalled();
  });
  expect(
    getApplicationCommand("Editar", "Desfazer"),
  ).toBeEnabled();
});

test("uses the same close decision for the application command and blocks it while resolving", async () => {
  const harness = projectWindowHarness();
  let finish!: () => void;
  harness.port.resolveClose = vi.fn(
    () =>
      new Promise<ProjectCloseResolution>((resolve) => {
        finish = () => resolve({ kind: "closed" });
      }),
  );

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={{
        ...projection,
        state: { ...projection.state, dirty: true },
      }}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={harness.dialog.port}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivo" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Fechar Projeto" }));
  expect(harness.port.requestClose).toHaveBeenCalledOnce();
  await waitFor(() =>
    expect(harness.dialog.present).toHaveBeenCalledWith({
      busy: false,
      kind: "projectCloseConfirmation",
    }),
  );

  act(() => harness.dialog.emit("saveAndClose"));
  expect(harness.port.resolveClose).toHaveBeenCalledWith("saveAndClose");
  expect(harness.dialog.present).toHaveBeenLastCalledWith({
    busy: true,
    kind: "projectCloseConfirmation",
  });
  expect(
    screen.getByRole("button", { name: "Exportar Lâmina", hidden: true }),
  ).toBeDisabled();

  await act(async () => finish());
  expect(
    screen.getByRole("menuitem", { name: "Editar" }),
  ).toBeDisabled();
});

test("sends Discard and resumes the unchanged Project after a conclusive save failure", async () => {
  const discardHarness = projectWindowHarness();
  const { unmount } = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={discardHarness.dialog.port}
      projectWindowPort={discardHarness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    discardHarness.emitCloseRequested();
    expect(discardHarness.dialog.present).toHaveBeenCalled();
  });
  act(() => discardHarness.dialog.emit("discardAndClose"));
  expect(discardHarness.port.resolveClose).toHaveBeenCalledWith(
    "discardAndClose",
  );
  unmount();

  const failureHarness = projectWindowHarness();
  failureHarness.port.resolveClose = vi.fn(async () => {
    throw new ProjectCloseError(
      "persisted_baseline_conflict",
      "O arquivo do Projeto foi alterado fora do MyAlbuns.",
    );
  });
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={failureHarness.dialog.port}
      projectWindowPort={failureHarness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    failureHarness.emitCloseRequested();
    expect(failureHarness.dialog.present).toHaveBeenCalled();
  });
  act(() => failureHarness.dialog.emit("saveAndClose"));

  await waitFor(() =>
    expect(failureHarness.dialog.present).toHaveBeenLastCalledWith({
      kind: "projectCloseFailure",
      message: "O arquivo do Projeto foi alterado fora do MyAlbuns.",
    }),
  );
  act(() => failureHarness.dialog.emit("dismissProjectCloseFailure"));
  await screen.findByRole("menuitem", { name: "Editar" });
  expect(
    getApplicationCommand("Editar", "Desfazer"),
  ).toBeEnabled();
});

test("never resumes or reports success after an indeterminate close save", async () => {
  const harness = projectWindowHarness();
  harness.port.resolveClose = vi.fn(async () => {
    throw new ProjectCloseError(
      "save_state_indeterminate",
      "Não foi possível confirmar qual revisão ficou no arquivo.",
    );
  });
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      projectDialogPort={harness.dialog.port}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    harness.emitCloseRequested();
    expect(harness.dialog.present).toHaveBeenCalled();
  });

  act(() => harness.dialog.emit("saveAndClose"));

  await waitFor(() =>
    expect(harness.dialog.present).toHaveBeenLastCalledWith({
      kind: "projectCloseFailure",
      message: "Não foi possível confirmar qual revisão ficou no arquivo.",
    }),
  );
  act(() => harness.dialog.emit("dismissProjectCloseFailure"));
  expect(
    screen.getByRole("menuitem", { name: "Editar" }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  ).toBeDisabled();
});

test("blocks only Project commands while its Export attempt is active", async () => {
  let emit!: (event: ExportProgressEvent) => void;
  let finish!: (outcome: ExportOutcome) => void;
  const completion = new Promise<ExportOutcome>((resolve) => {
    finish = resolve;
  });
  const controlledExportPort: ExportPort = {
    startSheet: (_sheetId, onEvent) => {
      emit = onEvent;
      return {
        completion,
        cancel: async () => "requested",
      };
    },
  };
  const projectSessionPort = projectSessionPortWithApply(async () => projection);
  const dialog = projectDialogHarness();
  projectSessionPort.undo = vi.fn(async () => projection);

  render(
    <ProjectWorkspace
      exportPort={controlledExportPort}
      projection={projection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  );
  expect(screen.getByRole("menuitem", { name: "Editar" })).toBeDisabled();

  act(() => {
    emit({ event: "started", cancellable: true });
  });
  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  expect(projectSessionPort.undo).not.toHaveBeenCalled();

  await act(async () => {
    finish({ status: "cancelled" });
    await completion;
  });

  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  expect(projectSessionPort.undo).not.toHaveBeenCalled();

  act(() => dialog.emit("dismissExport"));
  expect(getApplicationCommand("Editar", "Desfazer")).toBeEnabled();
});

test("forwards a fatal Canvas graphics diagnostic without interpreting it", () => {
  const onGraphicsUnavailable = vi.fn();
  const diagnostic: GraphicsDiagnostic = {
    supported: false,
    code: "webgl2_unavailable",
    renderer: "indisponível",
    reason: "O Canvas real não possui WebGL2.",
    limits: null,
  };
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
      onGraphicsUnavailable={onGraphicsUnavailable}
    />,
  );

  act(() => {
    canvasHarness.props?.onGraphicsUnavailable?.(diagnostic);
  });

  expect(onGraphicsUnavailable).toHaveBeenCalledWith(diagnostic);
});

test("restores accordion preferences after context changes and remounts", () => {
  const renderWorkspace = () =>
    render(
      <ProjectWorkspace
        exportPort={exportPort}
        projection={projection}
        projectSessionPort={projectSessionPortWithApply(async () => projection)}
        onProjectionChange={() => undefined}
      />,
    );

  const firstView = renderWorkspace();
  const albumInformation = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  expect(albumInformation).toHaveAttribute("aria-expanded", "true");

  fireEvent.click(albumInformation);
  expect(albumInformation).toHaveAttribute("aria-expanded", "false");

  act(() => useEditorView.setState({ selectedFrameId: "frame-001" }));
  expect(
    screen.getByRole("button", { name: "Design" }),
  ).toBeInTheDocument();

  act(() => useEditorView.setState({ selectedFrameId: null }));
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toHaveAttribute("aria-expanded", "false");

  firstView.unmount();
  renderWorkspace();
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toHaveAttribute("aria-expanded", "false");
});

test("uses the reference chrome and collapsible contextual sections", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.getByLabelText("MyAlbuns")).toBeInTheDocument();
  expect(
    screen.getByText("Álbum Horizonte", {
      selector: ".ui-application-header__identity strong",
    }),
  ).toBeInTheDocument();
  expect(screen.getByText("300×300 mm · 1 Lâmina")).toBeInTheDocument();
  expect(screen.queryByText("Intel(R) UHD Graphics")).not.toBeInTheDocument();
  expect(screen.queryByText("revisão 25")).not.toBeInTheDocument();
  expect(screen.queryByText("3 Fotos vinculadas")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("Zoom do Canvas")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Geral" })).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Grade de Lâminas" }),
  ).toBeInTheDocument();
});

test("shows the physical configuration projected from the opened Project", () => {
  const configuredProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      document: {
        displayUnit: "cm" as const,
        sheetWidthUm: 508_000,
        sheetHeightUm: 254_000,
        dpi: 240,
        bleedUm: 2_500,
        safetyUm: 5_000,
      },
      album: {
        ...projection.state.album,
        sheets: projection.state.album.sheets.map((sheet) => ({
          ...sheet,
          activeSides: "both",
        })),
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={configuredProjection}
      projectSessionPort={projectSessionPortWithApply(
        async () => configuredProjection,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const albumInformation = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );

  const sheetDimensions = within(
    albumInformation.getByRole("group", { name: "Dimensão da Lâmina" }),
  );
  expect(sheetDimensions.getByRole("textbox", { name: "Largura" })).toHaveValue(
    "50.8",
  );
  expect(sheetDimensions.getByRole("textbox", { name: "Altura" })).toHaveValue(
    "25.4",
  );

  const pageDimensions = within(
    albumInformation.getByRole("group", { name: "Dimensão da Página" }),
  );
  expect(pageDimensions.getByLabelText("Largura")).toHaveTextContent(
    "25.4 cm",
  );
  expect(pageDimensions.getByLabelText("Altura")).toHaveTextContent(
    "25.4 cm",
  );
  expect(pageDimensions.getByLabelText("Largura")).toHaveClass(
    "inspector-readout--integrated",
  );
  expect(albumInformation.getByLabelText("DPI")).toHaveValue("240");
  expect(albumInformation.getByRole("textbox", { name: "Sangria" })).toHaveValue(
    "0.25",
  );
  expect(
    albumInformation.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("0.5");
});

test("projects the pending Unidade across the Project Window without changing Album Design", async () => {
  const projectionWithBorder: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: false,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          frameBorder: { kind: "solid", rgb: "#2C2924", widthUm: 2_540 },
        },
      },
    },
    composition: {
      ...projection.composition,
      frameBorder: { kind: "solid", rgb: "#2C2924", widthUm: 2_540 },
    },
  };
  const apply = vi.fn(async () => projectionWithBorder);
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithBorder}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const informationSection = screen
    .getByRole("button", { name: "Informações do Álbum" })
    .closest("section") as HTMLElement;
  const designSection = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  const design = within(designSection);
  const designApply = design.getByRole("button", { name: "Aplicar" });

  fireEvent.change(within(informationSection).getByLabelText("Unidade"), {
    target: { value: "in" },
  });

  expect(await screen.findByText("11.811×11.811 pol · 1 Lâmina")).toBeVisible();
  expect(
    design.getByText("Borda padrão").closest("label"),
  ).toHaveTextContent("0.1 pol");
  expect(
    design.getByText("Espaço entre Frames").closest("label"),
  ).toHaveTextContent("0.236 pol");
  expect(designApply).toBeDisabled();
  expect(screen.getByText("salvo")).toBeVisible();
  expect(apply).not.toHaveBeenCalled();

  fireEvent.click(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  );
  expect(await screen.findByText("300×300 mm · 1 Lâmina")).toBeVisible();
  expect(
    design.getByText("Espaço entre Frames").closest("label"),
  ).toHaveTextContent("6 mm");
  expect(designApply).toBeDisabled();

  fireEvent.click(
    screen.getByRole("button", { name: "Informações do Álbum" }),
  );
  fireEvent.change(
    within(
      screen
        .getByRole("button", { name: "Informações do Álbum" })
        .closest("section") as HTMLElement,
    ).getByLabelText("Unidade"),
    { target: { value: "in" } },
  );
  expect(await screen.findByText("11.811×11.811 pol · 1 Lâmina")).toBeVisible();

  const otherProject: EditorProjection = {
    ...projectionWithBorder,
    state: {
      ...projectionWithBorder.state,
      projectId: "project-spike-002",
      document: {
        ...projectionWithBorder.state.document,
        displayUnit: "cm",
      },
    },
  };
  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={otherProject}
      projectSessionPort={projectSessionPortWithApply(async () => otherProject)}
      onProjectionChange={() => undefined}
    />,
  );
  expect(screen.getByText("30×30 cm · 1 Lâmina")).toBeVisible();
  expect(apply).not.toHaveBeenCalled();
});

test("clears pending Apply actions when their inspector forms are collapsed", async () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  const informationTrigger = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  const designTrigger = screen.getByRole("button", {
    name: "Design do Álbum",
  });
  const information = within(informationTrigger.closest("section") as HTMLElement);
  const design = within(designTrigger.closest("section") as HTMLElement);
  const informationApply = information.getByRole("button", { name: "Aplicar" });
  const designApply = design.getByRole("button", { name: "Aplicar" });

  fireEvent.change(information.getByLabelText("DPI"), {
    target: { value: "600" },
  });
  fireEvent.change(design.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });

  await waitFor(() => expect(informationApply).toBeEnabled());
  await waitFor(() => expect(designApply).toBeEnabled());

  fireEvent.click(informationTrigger);
  fireEvent.click(designTrigger);

  await waitFor(() => expect(informationApply).toBeDisabled());
  await waitFor(() => expect(designApply).toBeDisabled());
});

test("uses the current reference layout for the Album context", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  const albumInformationTrigger = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  const albumInformationSection = albumInformationTrigger
    .closest("section") as HTMLElement;
  const albumDesignTrigger = screen.getByRole("button", {
    name: "Design do Álbum",
  });
  const albumDesignSection = albumDesignTrigger
    .closest("section") as HTMLElement;
  const albumInformation = within(albumInformationSection);
  const albumDesign = within(albumDesignSection);
  const albumInformationApply = albumInformation.getByRole("button", {
    name: "Aplicar",
  });
  const albumDesignApply = albumDesign.getByRole("button", {
    name: "Aplicar",
  });

  expect(albumInformation.queryByText("Projeto")).not.toBeInTheDocument();
  expect(albumInformation.queryByText("Verificação")).not.toBeInTheDocument();
  expect(
    albumInformation.queryByLabelText("Nome do Projeto"),
  ).not.toBeInTheDocument();
  expect(
    albumInformation.queryByText("Frames placeholder"),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("button", { name: "Design do Álbum" }),
  ).toHaveAttribute("aria-expanded", "true");
  const albumDesignPreview = albumDesign.getByLabelText(
    "Prévia do padrão visual do Álbum",
  );
  expect(albumDesignPreview).toBeInTheDocument();
  expect(
    within(albumDesignPreview).getByRole("img", {
      name: "Composição do padrão visual do Álbum",
    }),
  ).toBeInTheDocument();
  expect(
    within(albumDesignPreview).queryByLabelText("Guias técnicas da Lâmina"),
  ).not.toBeInTheDocument();
  expect(
    albumDesignPreview.querySelector(".new-project-fixed-selection"),
  ).not.toBeInTheDocument();
  expect(
    within(albumDesignPreview).getByRole("group", {
      name: "Escopo do padrão visual do Álbum",
    }),
  ).toBeInTheDocument();
  expect(
    albumInformationSection.querySelector(
      '[data-placeholder-feature="album-end-sheet-settings"]',
    ),
  ).not.toBeInTheDocument();
  expect(
    albumInformationSection.querySelector(
      '[data-placeholder-feature="album-technical-area-settings"]',
    ),
  ).not.toBeInTheDocument();
  const compactControls = albumInformationSection.querySelector(
    ".document-compact-controls",
  ) as HTMLElement;
  expect(within(compactControls).getByLabelText("Unidade")).toBeInTheDocument();
  expect(within(compactControls).getByLabelText("DPI")).toBeInTheDocument();
  expect(albumInformationApply).toBeDisabled();
  expect(albumDesignApply).toBeDisabled();
  expect(albumInformationApply.closest(".inspector-section-header")).toContainElement(
    albumInformationTrigger,
  );
  expect(albumDesignApply.closest(".inspector-section-header")).toContainElement(
    albumDesignTrigger,
  );
  expect(albumDesignApply).not.toHaveAttribute("data-placeholder-feature");
  expect(albumInformation.getByText("Estrutura")).toBeInTheDocument();
  expect(albumInformation.getByText("Documento")).toBeInTheDocument();
  expect(albumInformation.getByText("Áreas técnicas")).toBeInTheDocument();
  expect(albumDesign.queryByText("Estrutura")).not.toBeInTheDocument();
  expect(albumDesign.queryByText("Documento")).not.toBeInTheDocument();
  expect(albumDesign.queryByText("Áreas técnicas")).not.toBeInTheDocument();
  expect(albumDesign.getByText("Padrões visuais")).toBeInTheDocument();
  expect(albumDesign.getByText("Padrão dos Frames")).toBeInTheDocument();
  expect(
    albumDesign.getByRole("slider", { name: "Espessura da Borda" }),
  ).toBeInTheDocument();
  expect(
    albumDesign.queryByRole("checkbox", { name: "Exibir borda" }),
  ).not.toBeInTheDocument();
  expect(albumDesign.getByLabelText("Cor do Background")).toBeInTheDocument();
});

test("edits and applies the complete Album design draft as one intent", async () => {
  const editableDesignProjection = {
    ...decorativeProjection,
    state: {
      ...decorativeProjection.state,
      album: {
        ...decorativeProjection.state.album,
        visualDefaults: {
          ...decorativeProjection.state.album.visualDefaults,
          overlay: { scope: "bothSides" as const, both: null },
        },
      },
    },
    composition: {
      ...decorativeProjection.composition,
      sheets: decorativeProjection.composition.sheets.map((sheet) => ({
        ...sheet,
        overlays: [],
      })),
    },
  } satisfies EditorProjection;
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    editableDesignProjection,
  );

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={editableDesignProjection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  const applyDesign = albumDesign.getByRole("button", { name: "Aplicar" });
  expect(applyDesign).toBeDisabled();

  const scopeControls = within(
    albumDesign.getByLabelText("Prévia do padrão visual do Álbum"),
  ).getByRole("group", { name: "Escopo do padrão visual do Álbum" });
  expect(within(scopeControls).getAllByRole("button")).toHaveLength(3);
  expect(
    within(scopeControls).getByRole("button", { name: "Ambos os lados" }),
  ).toHaveAttribute("aria-pressed", "true");

  fireEvent.click(albumDesign.getByRole("button", { name: "Lado esquerdo" }));
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  expect(
    albumDesign.queryByRole("button", { name: "Escolher Overlay" }),
  ).not.toBeInTheDocument();
  expect(
    albumDesign.queryByRole("button", { name: /Abrir mais opções/ }),
  ).not.toBeInTheDocument();
  expect(
    albumDesign.getByRole("group", { name: "Opções de Background" }),
  ).toBeInTheDocument();

  fireEvent.click(
    albumDesign.getByRole("button", {
      name: "Escolher Decorativo para Overlay",
    }),
  );
  fireEvent.click(
    albumDesign.getByRole("menuitem", {
      name: "Usar Overlay Overlay translúcido.png",
    }),
  );
  expect(
    albumDesign.queryByRole("menu", { name: "Decorativos para Overlay" }),
  ).not.toBeInTheDocument();
  expect(albumDesign.queryByRole("dialog")).not.toBeInTheDocument();
  const frame = albumDesign.getByLabelText("Frame demonstrativo esquerdo 1");
  const overlay = albumDesign.getByLabelText("Overlay do lado esquerdo");
  expect(
    frame.compareDocumentPosition(overlay) & Node.DOCUMENT_POSITION_FOLLOWING,
  ).toBeTruthy();
  fireEvent.change(albumDesign.getByLabelText("Cor da Borda"), {
    target: { value: "#2c2924" },
  });
  const borderWidth = albumDesign.getByRole("slider", {
    name: "Espessura da Borda",
  });
  expect(borderWidth).toHaveAttribute("min", "0");
  fireEvent.change(borderWidth, { target: { value: "1250" } });

  expect(applyDesign).toBeEnabled();
  fireEvent.click(applyDesign);

  await waitFor(() =>
    expect(apply).toHaveBeenCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        background: {
          scope: "perSide",
          left: { kind: "color", rgb: "#F7F5F0" },
          right: { kind: "color", rgb: "#FFFFFF" },
        },
        overlay: {
          scope: "perSide",
          left: { kind: "media", mediaId: "decorative-overlay" },
          right: null,
        },
        frameBorder: {
          kind: "solid",
          rgb: "#2C2924",
          widthUm: 1_250,
        },
      },
    }),
  );
});

test("prevents re-entering Album Design Apply while its mutation is pending", async () => {
  const pending = deferredProjection();
  const apply = vi.fn<ProjectSessionPort["apply"]>(() => pending.promise);

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const albumDesign = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  fireEvent.change(albumDesign.getByLabelText("Cor do Background"), {
    target: { value: "#f7f5f0" },
  });
  const applyDesign = albumDesign.getByRole("button", { name: "Aplicar" });
  expect(applyDesign).toBeEnabled();

  fireEvent.click(applyDesign);
  fireEvent.click(applyDesign);

  expect(applyDesign).toBeDisabled();
  expect(apply).toHaveBeenCalledOnce();

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });
  await waitFor(() => expect(apply).toHaveBeenCalledOnce());
});

test("maps Borda zero to none and a positive value back to solid", async () => {
  const projectionWithBorder: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          frameBorder: { kind: "solid", rgb: "#C5A46D", widthUm: 2_500 },
        },
      },
    },
    composition: {
      ...projection.composition,
      frameBorder: { kind: "solid", rgb: "#C5A46D", widthUm: 2_500 },
    },
  };
  const projectionWithoutBorder: EditorProjection = {
    ...projectionWithBorder,
    state: {
      ...projectionWithBorder.state,
      album: {
        ...projectionWithBorder.state.album,
        visualDefaults: {
          ...projectionWithBorder.state.album.visualDefaults,
          frameBorder: { kind: "none" },
        },
      },
    },
    composition: {
      ...projectionWithBorder.composition,
      frameBorder: { kind: "none" },
    },
  };
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () =>
    projectionWithoutBorder,
  );
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithBorder}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const design = within(
    screen
      .getByRole("button", { name: "Design do Álbum" })
      .closest("section") as HTMLElement,
  );
  const borderWidth = design.getByRole("slider", {
    name: "Espessura da Borda",
  });
  const applyDesign = design.getByRole("button", { name: "Aplicar" });

  fireEvent.change(borderWidth, { target: { value: "0" } });
  expect(design.getByText("sem borda")).toBeVisible();
  fireEvent.click(applyDesign);
  await waitFor(() =>
    expect(apply).toHaveBeenLastCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        ...projectionWithBorder.state.album.visualDefaults,
        frameBorder: { kind: "none" },
      },
    }),
  );

  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithoutBorder}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );
  fireEvent.change(
    design.getByRole("slider", { name: "Espessura da Borda" }),
    { target: { value: "1250" } },
  );
  expect(design.getByText("1.25 mm")).toBeVisible();
  fireEvent.click(applyDesign);
  await waitFor(() =>
    expect(apply).toHaveBeenLastCalledWith({
      kind: "setVisualDefaults",
      visualDefaults: {
        ...projectionWithBorder.state.album.visualDefaults,
        frameBorder: {
          kind: "solid",
          rgb: "#C5A46D",
          widthUm: 1_250,
        },
      },
    }),
  );
});

test("keeps Espaço entre Frames as a preview-only placeholder", () => {
  const apply = vi.fn<ProjectSessionPort["apply"]>(async () => projection);
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const designSection = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  const design = within(designSection);
  const gap = design.getByRole("slider", { name: "Espaço entre Frames" });
  const secondFrame = design.getByLabelText("Frame demonstrativo esquerdo 2");
  const initialSecondFrameX = Number(secondFrame.getAttribute("x"));
  const applyDesign = design.getByRole("button", { name: "Aplicar" });

  expect(gap.closest("label")).toHaveAttribute(
    "data-placeholder-feature",
    "album-design-frame-gap",
  );
  fireEvent.change(gap, { target: { value: "18000" } });

  expect(design.getByText("18 mm")).toBeVisible();
  expect(Number(secondFrame.getAttribute("x"))).toBeGreaterThan(
    initialSecondFrameX,
  );
  expect(applyDesign).toBeDisabled();
  expect(apply).not.toHaveBeenCalled();
});

test("coordinates Decorative popups, placeholder import and focus restoration", async () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={decorativeProjection}
      projectSessionPort={projectSessionPortWithApply(async () =>
        decorativeProjection
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const designSection = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  const design = within(designSection);
  const backgroundTrigger = design.getByRole("button", {
    name: "Escolher Decorativo para Background",
  });
  const overlayTrigger = design.getByRole("button", {
    name: /Decorativo do Overlay: Overlay translúcido\.png/,
  });

  fireEvent.click(backgroundTrigger);
  const backgroundMenu = design.getByRole("menu", {
    name: "Decorativos para Background",
  });
  const importPlaceholder = within(backgroundMenu).getByRole("menuitem", {
    name: "Importar Decorativo",
  });
  expect(importPlaceholder).toBeDisabled();
  expect(importPlaceholder).toHaveAttribute(
    "data-placeholder-feature",
    "import-decorative-files",
  );

  fireEvent.click(overlayTrigger);
  expect(backgroundMenu).not.toBeInTheDocument();
  const overlayMenu = design.getByRole("menu", {
    name: "Decorativos para Overlay",
  });
  await waitFor(() =>
    expect(within(overlayMenu).getAllByRole("menuitem")[0]).toHaveFocus(),
  );

  fireEvent.keyDown(document, { key: "Escape" });
  expect(overlayMenu).not.toBeInTheDocument();
  expect(overlayTrigger).toHaveFocus();

  fireEvent.click(backgroundTrigger);
  fireEvent.pointerDown(design.getByText("Padrões visuais"));
  expect(
    design.queryByRole("menu", { name: "Decorativos para Background" }),
  ).not.toBeInTheDocument();
  await waitFor(() => expect(backgroundTrigger).toHaveFocus());
});

test("presents an empty per-side Overlay as absent", () => {
  const projectionWithoutOverlay: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          overlay: { scope: "perSide", left: null, right: null },
        },
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithoutOverlay}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithoutOverlay,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const visualDefaults = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  expect(
    within(visualDefaults).getByRole("button", { name: "Sem Overlay" }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(
    visualDefaults.querySelector(".visual-default-picker__preview--none"),
  ).toBeInTheDocument();
});

test("does not present divergent per-side Overlays as absent", () => {
  const projectionWithMixedOverlay: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        visualDefaults: {
          ...projection.state.album.visualDefaults,
          overlay: {
            scope: "perSide",
            left: { kind: "media", mediaId: "decorative-overlay" },
            right: null,
          },
        },
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithMixedOverlay}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithMixedOverlay,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const visualDefaults = screen
    .getByRole("button", { name: "Design do Álbum" })
    .closest("section") as HTMLElement;
  // Um lado tem Overlay e o outro não: o escopo é misto, não é ausência.
  expect(
    within(visualDefaults).getByRole("button", { name: "Sem Overlay" }),
  ).toHaveAttribute("aria-pressed", "false");
});

test("confirms and applies Album information as one authoritative Project change", async () => {
  const initialProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      revision: 0,
      savedRevision: 0,
      dirty: false,
      canUndo: false,
      canRedo: false,
    },
  };
  const changedProjection: EditorProjection = {
    ...initialProjection,
    state: {
      ...initialProjection.state,
      document: {
        ...initialProjection.state.document,
        dpi: 600,
      },
      revision: 1,
      dirty: true,
      canUndo: true,
    },
  };
  const apply = vi.fn(async () => changedProjection);
  const projectSessionPort = projectSessionPortWithApply(apply);
  const dialog = projectDialogHarness();
  const onProjectionChange = vi.fn();

  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={initialProjection}
      projectDialogPort={dialog.port}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const albumInformationTrigger = screen.getByRole("button", {
    name: "Informações do Álbum",
  });
  if (albumInformationTrigger.getAttribute("aria-expanded") !== "true") {
    fireEvent.click(albumInformationTrigger);
  }
  const albumInformationBeforeApply = within(
    albumInformationTrigger.closest("section") as HTMLElement,
  );
  const input = albumInformationBeforeApply.getByRole("textbox", {
    name: "DPI",
  });
  fireEvent.change(input, { target: { value: "600" } });
  expect(apply).not.toHaveBeenCalled();
  await waitFor(() =>
    expect(
      albumInformationBeforeApply.getByRole("button", { name: "Aplicar" }),
    ).toBeEnabled(),
  );

  await act(async () => {
    fireEvent.click(
      albumInformationBeforeApply.getByRole("button", { name: "Aplicar" }),
    );
    await Promise.resolve();
  });

  expect(apply).not.toHaveBeenCalled();
  expect(dialog.present).toHaveBeenLastCalledWith(
    expect.objectContaining({
      busy: false,
      kind: "albumInformationConfirmation",
    }),
  );

  await act(async () => {
    dialog.emit("confirmAlbumInformation");
    await Promise.resolve();
    await Promise.resolve();
  });

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "setAlbumInformation",
    information: {
      displayUnit: "mm",
      sheetWidthUm: initialProjection.state.document.sheetWidthUm,
      sheetHeightUm: initialProjection.state.document.sheetHeightUm,
      dpi: 600,
      bleedUm: initialProjection.state.document.bleedUm,
      safetyUm: initialProjection.state.document.safetyUm,
      firstSheet: "double",
      lastSheet: "double",
    },
  });
  expect(onProjectionChange).toHaveBeenCalledWith(changedProjection);

  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={changedProjection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const albumInformation = within(
    screen
      .getByRole("button", { name: "Informações do Álbum" })
      .closest("section") as HTMLElement,
  );
  expect(albumInformation.getByRole("textbox", { name: "DPI" })).toHaveValue(
    "600",
  );
  expect(getApplicationCommand("Editar", "Desfazer")).toBeEnabled();
});

test("saves the visible revision and applies the authoritative saved projection", async () => {
  const savedProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      savedRevision: projection.state.revision,
      dirty: false,
      canUndo: true,
    },
  };
  const save = vi.fn<ProjectSessionPort["save"]>(async () => ({
    outcome: {
      kind: "saved",
      revision: savedProjection.state.revision,
    },
    projection: savedProjection,
  }));
  const projectSessionPort = projectSessionPortWithApply(
    async () => projection,
  );
  projectSessionPort.save = save;
  const onProjectionChange = vi.fn();

  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );

  const saveCommand = getApplicationCommand("Arquivo", "Salvar");
  await act(async () => {
    fireEvent.click(saveCommand);
    await Promise.resolve();
  });

  expect(save).toHaveBeenCalledWith(projection.state.revision);
  expect(onProjectionChange).toHaveBeenCalledWith(savedProjection);

  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={savedProjection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  expect(getApplicationCommand("Editar", "Desfazer")).toBeEnabled();
});

test("uses Ctrl+S for Project save and prevents the browser default", async () => {
  const save = vi.fn<ProjectSessionPort["save"]>(async () => ({
    outcome: { kind: "saved", revision: projection.state.revision },
    projection: {
      ...projection,
      state: {
        ...projection.state,
        savedRevision: projection.state.revision,
        dirty: false,
      },
    },
  }));
  const projectSessionPort = projectSessionPortWithApply(
    async () => projection,
  );
  projectSessionPort.save = save;
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );
  const shortcut = new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ctrlKey: true,
    key: "s",
  });

  await act(async () => {
    window.dispatchEvent(shortcut);
    await Promise.resolve();
  });

  expect(shortcut.defaultPrevented).toBe(true);
  expect(save).toHaveBeenCalledOnce();
  expect(save).toHaveBeenCalledWith(projection.state.revision);
});

test("shows the localized Project save failure", async () => {
  const projectSessionPort = projectSessionPortWithApply(
    async () => projection,
  );
  projectSessionPort.save = vi.fn(async () => {
    throw new SaveProjectError(
      "persisted_baseline_conflict",
      "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
    );
  });
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPort}
      onProjectionChange={() => undefined}
    />,
  );

  const saveCommand = getApplicationCommand("Arquivo", "Salvar");
  await act(async () => {
    fireEvent.click(saveCommand);
    await Promise.resolve();
  });

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
  );
});

test("renders each Grade item from its own composed sheet", () => {
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={twoSheetProjection}
      projectSessionPort={projectSessionPortWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 01" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 02" }),
  ).toBeInTheDocument();
  const sheetGrid = view.container.querySelector(".sheet-grid") as HTMLElement;
  expect(within(sheetGrid).getAllByRole("img")).toHaveLength(2);
});

test("presents the Grade with reference metadata and navigation state", () => {
  const projectionWithProjectedPageNumbers: EditorProjection = {
    ...twoSheetProjection,
    state: {
      ...twoSheetProjection.state,
      album: {
        ...twoSheetProjection.state.album,
        sheets: twoSheetProjection.state.album.sheets.map((sheet, index) => ({
          ...sheet,
          pageNumbers: index === 0 ? [7, 8] : [12, 13],
        })),
      },
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithProjectedPageNumbers}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithProjectedPageNumbers,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const gradeTrigger = screen.getByRole("button", {
    name: "Grade de Lâminas",
  });
  expect(
    gradeTrigger.querySelector(".inspector-section-meta"),
  ).toHaveTextContent("2");

  const tiles = Array.from(
    view.container.querySelectorAll<HTMLElement>(".sheet-tile"),
  );
  expect(tiles).toHaveLength(2);
  expect(tiles[0]).toHaveAttribute("aria-current", "true");
  expect(tiles[1]).not.toHaveAttribute("aria-current");
  expect(tiles[0]).toHaveAttribute("data-active-sides", "both");
  expect(tiles[0]).toHaveAccessibleName(
    "Ir para Lâmina 01, Páginas 7–8",
  );
  expect(tiles[0].querySelector(".sheet-tile__number")).toHaveTextContent(
    "01",
  );
  expect(tiles[0].querySelector(".sheet-tile__pages")?.textContent).toBe(
    "7–8",
  );
  expect(tiles[1].querySelector(".sheet-tile__pages")?.textContent).toBe(
    "12–13",
  );
});

test("shows Page numbers instead of cover and final aliases", () => {
  const projectionWithSinglePageEnds: EditorProjection = {
    ...twoSheetProjection,
    state: {
      ...twoSheetProjection.state,
      album: {
        ...twoSheetProjection.state.album,
        sheets: twoSheetProjection.state.album.sheets.map((sheet, index) => ({
          ...sheet,
          activeSides: index === 0 ? "right" : "left",
          pageNumbers: [index + 1],
        })),
      },
    },
    composition: {
      ...twoSheetProjection.composition,
      sheets: twoSheetProjection.composition.sheets.map((sheet, index) => {
        const widthUm = sheet.widthUm / 2;
        return {
          ...sheet,
          activeSides: index === 0 ? "right" : "left",
          widthUm,
          base: {
            ...sheet.base,
            drawRect: { ...sheet.base.drawRect, width: widthUm },
          },
          backgrounds: sheet.backgrounds.map((background) => ({
            ...background,
            drawRect: { ...background.drawRect, width: widthUm },
          })),
          frames: [],
          overlays: [],
        };
      }),
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithSinglePageEnds}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithSinglePageEnds,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  const tiles = Array.from(
    view.container.querySelectorAll<HTMLElement>(".sheet-tile"),
  );
  expect(
    tiles.map(
      (tile) => tile.querySelector(".sheet-tile__pages")?.textContent,
    ),
  ).toEqual(["1", "2"]);
  expect(tiles[0]).toHaveAccessibleName(
    "Ir para Lâmina 01, Lâmina inicial, Página 1",
  );
  expect(tiles[1]).toHaveAccessibleName(
    "Ir para Lâmina 02, Lâmina final, Página 2",
  );
  expect(
    tiles[0].style.getPropertyValue("--sheet-inactive-side-gradient"),
  ).toBe(
    "linear-gradient(to right, #faf9f6 0%, #ebe3d8 58%, #cec2b2 100%)",
  );
  expect(
    tiles[1].style.getPropertyValue("--sheet-inactive-side-gradient"),
  ).toBe(
    "linear-gradient(to left, #faf9f6 0%, #ebe3d8 58%, #cec2b2 100%)",
  );
});

test("uses reduced Cache previews in the media panel and Canvas", () => {
  const mediaPreviewUrls = {
    "media-001": "asset://localhost/cache/media-001.jpg",
  };
  const mediaPreviews = {
    "media-001": {
      mediaId: "media-001",
      state: "ready" as const,
      url: mediaPreviewUrls["media-001"],
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      mediaPreviews={mediaPreviews}
      onProjectionChange={() => undefined}
    />,
  );

  expect(
    view.container.querySelector<HTMLImageElement>(
      '.media-thumb img[src="asset://localhost/cache/media-001.jpg"]',
    ),
  ).not.toBeNull();
  expect(canvasHarness.props?.mediaPreviewUrls).toEqual(
    mediaPreviewUrls,
  );
});

test("merges only Panel viewport and one-row preload margin with Canvas demand", async () => {
  const onMediaDemandChange = vi.fn();
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onMediaDemandChange={onMediaDemandChange}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onMediaDemandChange?.({
      visibleMediaIds: ["media-003"],
      preloadMediaIds: ["decorative-not-in-panel"],
    });
    emitPanelIntersections("0px", {
      "media-001": true,
      "media-002": false,
      "media-003": false,
    });
    emitPanelIntersections("122px 0px", {
      "media-001": true,
      "media-002": true,
      "media-003": false,
    });
  });

  await waitFor(() =>
    expect(onMediaDemandChange).toHaveBeenLastCalledWith({
      visibleMediaIds: ["media-003", "media-001"],
      preloadMediaIds: ["decorative-not-in-panel", "media-002"],
    }),
  );
});

test("preloads imported Decoratives for Album design before they are used", async () => {
  const onMediaDemandChange = vi.fn();
  const projectionWithUnusedDecorative: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      album: {
        ...projection.state.album,
        media: [
          ...projection.state.album.media,
          {
            id: "decorative-unused",
            kind: "decorative",
            name: "Textura ainda não usada.png",
            palette: ["#E8E1D6", "#B8AA96", "#81705A"],
            sourceHeightPx: 1_200,
            sourceWidthPx: 1_600,
          },
        ],
      },
    },
  };

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projectionWithUnusedDecorative}
      projectSessionPort={projectSessionPortWithApply(
        async () => projectionWithUnusedDecorative,
      )}
      onMediaDemandChange={onMediaDemandChange}
      onProjectionChange={() => undefined}
    />,
  );

  await waitFor(() =>
    expect(onMediaDemandChange).toHaveBeenCalledWith({
      visibleMediaIds: [],
      preloadMediaIds: ["decorative-unused"],
    }),
  );
});

test("resizes media cards without restarting Panel demand observers", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  const observerCount = observedViewports.length;
  fireEvent.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  fireEvent.change(
    screen.getByRole("slider", { name: "Tamanho das miniaturas" }),
    { target: { value: "124" } },
  );

  expect(observerCount).toBe(2);
  expect(observedViewports).toHaveLength(observerCount);
});

test("shares one Decorative Cache preview across Panel, Canvas, and Grade", () => {
  const mediaPreviewUrls = {
    "decorative-overlay": decorativePreviewUrl,
  };
  const mediaPreviews = {
    "decorative-overlay": {
      mediaId: "decorative-overlay",
      state: "ready" as const,
      url: decorativePreviewUrl,
    },
  };
  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={decorativeProjection}
      projectSessionPort={projectSessionPortWithApply(
        async () => decorativeProjection,
      )}
      mediaPreviews={mediaPreviews}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));

  expect(
    screen.getByRole("button", { name: /^Overlay translúcido\.png/ }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /^Serra ao amanhecer\.jpg/ }),
  ).not.toBeInTheDocument();
  expect(
    view.container.querySelector<HTMLImageElement>(
      `.media-thumb img[src="${decorativePreviewUrl}"]`,
    ),
  ).not.toBeNull();
  expect(
    view.container.querySelector<SVGImageElement>(
      `[data-preview-overlay-id="decorative-overlay"][href="${decorativePreviewUrl}"]`,
    ),
  ).not.toBeNull();
  expect(canvasHarness.props?.mediaPreviewUrls).toEqual(mediaPreviewUrls);
});

test("renders derived media usage as the thumbnail opacity state", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  const usedMedia = screen.getByRole("button", {
    name: "Serra ao amanhecer.jpg. Já usada",
  });
  expect(usedMedia).toHaveAttribute("data-used", "true");
  expect(usedMedia).not.toHaveTextContent("Serra ao amanhecer.jpg");
});

test("centers a Grade navigation target in the visible Canvas", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={twoSheetProjection}
      projectSessionPort={projectSessionPortWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      scale: 0.5,
    });
  });
  fireEvent.click(screen.getByText("02").closest("button")!);

  const targetCenter =
    canvasHarness.props!.continuousCanvasLayout.entriesAtScale(0.5)[1].center;
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - targetCenter * 0.5,
  );
});

test("completes Grade navigation requested before Canvas metrics exist", () => {
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={twoSheetProjection}
      projectSessionPort={projectSessionPortWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByText("02").closest("button")!);
  expect(useEditorView.getState().viewport.offsetX).toBe(42);

  act(() => {
    canvasHarness.props?.onCanvasMetricsChange?.({
      width: 1_000,
      scale: 0.5,
    });
  });

  const targetCenter =
    canvasHarness.props!.continuousCanvasLayout.entriesAtScale(0.5)[1].center;
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - targetCenter * 0.5,
  );
  expect(useEditorView.getState().focusedSheetId).toBe("sheet-002");
  expect(useEditorView.getState().centeredSheetId).toBe("sheet-002");
});

test("resizes both workspace panels with persistent splitters", () => {
  const firstView = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.queryByText("Canvas contÃ­nuo")).not.toBeInTheDocument();

  const verticalSplitter = screen.getByRole("separator", {
    name: "Redimensionar Painel contextual",
  });
  const horizontalSplitter = screen.getByRole("separator", {
    name: "Redimensionar Painel de imagens",
  });
  const workspace = verticalSplitter.parentElement!;
  vi.spyOn(workspace, "getBoundingClientRect").mockReturnValue({
    left: 0,
    right: 1_200,
    top: 0,
    bottom: 800,
    width: 1_200,
    height: 800,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });

  fireEvent.pointerDown(verticalSplitter, { pointerId: 1 });
  fireEvent.pointerMove(window, { clientX: 850, clientY: 0 });
  fireEvent.pointerUp(window, { pointerId: 1 });

  fireEvent.pointerDown(horizontalSplitter, { pointerId: 2 });
  fireEvent.pointerMove(window, { clientX: 0, clientY: 600 });
  fireEvent.pointerUp(window, { pointerId: 2 });

  expect(workspace.getAttribute("style")).toContain(
    "--inspector-width: 350px",
  );
  expect(workspace.getAttribute("style")).toContain(
    "--media-panel-height: 200px",
  );
  expect(localStorage.getItem("myalbuns.workspace.inspector-width")).toBe(
    "350",
  );
  expect(localStorage.getItem("myalbuns.workspace.media-panel-height")).toBe(
    "200",
  );

  firstView.unmount();
  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );
  expect(
    screen
      .getByRole("separator", {
        name: "Redimensionar Painel contextual",
      })
      .parentElement?.getAttribute("style"),
  ).toContain("--inspector-width: 350px");
});

test("commits a slider zoom once without flashing a global busy state", async () => {
  const pending = deferredProjection();
  const apply = vi.fn(() => pending.promise);
  useEditorView.setState({ selectedFrameId: "frame-001" });

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const slider = screen.getByRole("slider", { name: "Zoom da Foto" });
  const exportButton = screen.getByRole("button", {
    name: "Exportar Lâmina",
  });

  fireEvent.pointerDown(slider);
  fireEvent.change(slider, { target: { value: "112" } });
  fireEvent.change(slider, { target: { value: "125" } });

  expect(apply).not.toHaveBeenCalled();

  fireEvent.pointerUp(slider);

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: 0.25,
  });
  expect(screen.queryByText("Aplicando alteração")).not.toBeInTheDocument();
  expect(exportButton).toBeEnabled();

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });
});

test("updates the contextual Zoom slider during a Canvas gesture", () => {
  const apply = vi.fn(async () => projection);
  useEditorView.setState({ selectedFrameId: "frame-001" });

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const slider = screen.getByRole("slider", { name: "Zoom da Foto" });
  expect(slider).toHaveValue("100");

  act(() => {
    canvasHarness.props?.onTransformPreview?.({
      frameId: "frame-001",
      panX: 0.35,
      panY: -0.2,
      zoom: 1.25,
    });
  });

  expect(slider).toHaveValue("125");
  expect(screen.getByText("Pan horizontal").parentElement).toHaveTextContent(
    "35%",
  );
  expect(apply).not.toHaveBeenCalled();

  act(() => {
    canvasHarness.props?.onTransformPreview?.(null);
  });

  expect(slider).toHaveValue("100");
  expect(screen.getByText("Pan horizontal").parentElement).toHaveTextContent(
    "0%",
  );
});

test("discards a live Canvas value when its commit fails", async () => {
  const apply = vi.fn(async () => {
    throw new Error("Falha simulada");
  });
  useEditorView.setState({ selectedFrameId: "frame-001" });

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  const slider = screen.getByRole("slider", { name: "Zoom da Foto" });
  act(() => {
    canvasHarness.props?.onTransformPreview?.({
      frameId: "frame-001",
      panX: 0,
      panY: 0,
      zoom: 1.25,
    });
  });
  expect(slider).toHaveValue("125");

  let accepted = true;
  await act(async () => {
    accepted =
      (await canvasHarness.props?.onTransformCommit({
        frameId: "frame-001",
        deltaPanX: 0,
        deltaPanY: 0,
        deltaZoom: 0.25,
      })) ?? true;
  });

  expect(accepted).toBe(false);
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Falha simulada",
  );
  expect(slider).toHaveValue("100");
});

test("does not let an old Project completion clear a new slider draft", async () => {
  const pending = deferredProjection();
  const oldApply = vi.fn(() => pending.promise);
  const otherProject = {
    ...projection,
    state: {
      ...projection.state,
      projectId: "project-spike-002",
    },
  };
  const newApply = vi.fn(async () => otherProject);
  const onProjectionChange = vi.fn();
  const oldProjectSessionPort = projectSessionPortWithApply(oldApply);
  const newProjectSessionPort = projectSessionPortWithApply(newApply);
  useEditorView.setState({ selectedFrameId: "frame-001" });

  const view = render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={oldProjectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  const oldSlider = screen.getByRole("slider", {
    name: "Zoom da Foto",
  });
  fireEvent.pointerDown(oldSlider);
  fireEvent.change(oldSlider, { target: { value: "125" } });
  fireEvent.pointerUp(oldSlider);
  expect(oldApply).toHaveBeenCalledOnce();

  view.rerender(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={otherProject}
      projectSessionPort={newProjectSessionPort}
      onProjectionChange={onProjectionChange}
    />,
  );
  act(() => useEditorView.setState({ selectedFrameId: "frame-001" }));
  const newSlider = screen.getByRole("slider", {
    name: "Zoom da Foto",
  });
  fireEvent.pointerDown(newSlider);
  fireEvent.change(newSlider, { target: { value: "130" } });
  expect(newSlider).toHaveValue("130");

  await act(async () => {
    pending.resolve(projection);
    await pending.promise;
  });

  expect(newSlider).toHaveValue("130");
  expect(onProjectionChange).not.toHaveBeenCalled();

  fireEvent.pointerUp(newSlider);
  expect(newApply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0,
    deltaPanY: 0,
    deltaZoom: 0.3,
  });
});

test("uses the Canvas-centered sheet for a media double click", () => {
  const apply = vi.fn(async () => twoSheetProjection);

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={twoSheetProjection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCenteredSheetChange?.("sheet-002");
  });
  fireEvent.doubleClick(
    screen.getByRole("button", { name: "Campo.jpg" }),
  );

  expect(apply).toHaveBeenCalledWith({
    kind: "fillLeftmostPlaceholder",
    sheetId: "sheet-002",
    mediaId: "media-002",
  });
});

test("starts Exportação for the Canvas-centered Lâmina even while focus remains on another Lâmina", () => {
  const startSheet = vi.fn<ExportPort["startSheet"]>(() => ({
    completion: Promise.resolve({
      status: "completed",
      result: { widthPx: 600, heightPx: 300 },
    }),
    cancel: async () => "not_found",
  }));

  render(
    <ProjectWorkspace
      exportPort={{ startSheet }}
      projection={twoSheetProjection}
      projectSessionPort={projectSessionPortWithApply(async () =>
        twoSheetProjection
      )}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCenteredSheetChange?.("sheet-002");
  });
  expect(useEditorView.getState().focusedSheetId).toBe("sheet-001");
  fireEvent.click(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  );

  expect(startSheet).toHaveBeenCalledWith("sheet-002", expect.any(Function));
});

test("forwards simultaneous Canvas Pan and Zoom as one intent", () => {
  const apply = vi.fn(async () => projection);

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  canvasHarness.props?.onTransformCommit({
    frameId: "frame-001",
    deltaPanX: 0.35,
    deltaPanY: -0.2,
    deltaZoom: 0.12,
  });

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({
    kind: "transformPhoto",
    frameId: "frame-001",
    deltaPanX: 0.35,
    deltaPanY: -0.2,
    deltaZoom: 0.12,
  });
});

test("serializes Project mutations so projections cannot arrive out of order", async () => {
  const first = deferredProjection();
  const second = deferredProjection();
  const apply = vi
    .fn<ProjectSessionPort["apply"]>()
    .mockImplementationOnce(() => first.promise)
    .mockImplementationOnce(() => second.promise);
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPort={exportPort}
      projection={projection}
      projectSessionPort={projectSessionPortWithApply(apply)}
      onProjectionChange={onProjectionChange}
    />,
  );

  canvasHarness.props?.onTransformCommit({
    frameId: "frame-001",
    deltaPanX: 0.1,
    deltaPanY: 0,
    deltaZoom: 0,
  });
  canvasHarness.props?.onTransformCommit({
    frameId: "frame-001",
    deltaPanX: 0.2,
    deltaPanY: 0,
    deltaZoom: 0,
  });

  await act(async () => {
    await Promise.resolve();
  });
  expect(apply).toHaveBeenCalledOnce();

  const firstProjection = {
    ...projection,
    state: { ...projection.state, revision: 26 },
  };
  await act(async () => {
    first.resolve(firstProjection);
    await first.promise;
  });

  expect(onProjectionChange).toHaveBeenLastCalledWith(firstProjection);
  expect(apply).toHaveBeenCalledTimes(2);

  const secondProjection = {
    ...projection,
    state: { ...projection.state, revision: 27 },
  };
  await act(async () => {
    second.resolve(secondProjection);
    await second.promise;
  });

  expect(onProjectionChange).toHaveBeenLastCalledWith(secondProjection);
});
