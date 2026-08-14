import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { beforeEach, expect, test, vi } from "vitest";

import type {
  ExportOutcome,
  ExportPipelinePort,
  ExportProgressEvent,
  MediaPreviewDemand,
  ProjectCloseResolution,
  ProjectCorePort,
  ProjectWindowPort,
} from "../application/projectPorts";
import {
  ProjectCloseError,
  SaveProjectError,
} from "../application/projectPorts";
import type { GraphicsDiagnostic } from "../application/graphics";
import type { EditorProjection } from "../domain/project";
import { useEditorView } from "../state/editorView";
import {
  createTwoSheetProjection,
  representativeProjection,
} from "../test/projectFixtures";
import type {
  CanvasMetrics,
  PhotoTransformDelta,
  PhotoTransformPreview,
} from "./AlbumCanvas";
import type { ContinuousCanvasLayout } from "./canvasGeometry";
import { ProjectWorkspace as ProjectWorkspaceView } from "./ProjectWorkspace";
import { useProjectMutationRunner } from "./useProjectMutationRunner";

const canvasHarness = vi.hoisted(() => ({
  props: null as null | {
    continuousCanvasLayout: ContinuousCanvasLayout;
    mediaPreviewUrls?: Readonly<Record<string, string>>;
    onMediaDemandChange?(demand: MediaPreviewDemand): void;
    onCanvasMetricsChange?(metrics: CanvasMetrics): void;
    onCenteredSheetChange?(sheetId: string): void;
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

const exportPipelinePort: ExportPipelinePort = {
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
    port,
    emitCloseRequested: () => closeRequested?.(),
  };
}

function projectCorePortWithApply(
  apply: ProjectCorePort["apply"],
): ProjectCorePort {
  return {
    load: async () => projection,
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
  "runProjectMutation" | "projectWindowPort"
> & {
  projectCorePort: ProjectCorePort;
  projectWindowPort?: ProjectWindowPort;
};

function ProjectWorkspace({
  projectCorePort,
  projectWindowPort = inertProjectWindowPort,
  projection,
  ...props
}: TestProjectWorkspaceProps) {
  const runProjectMutation = useProjectMutationRunner(
    projection.state.projectId,
    projectCorePort,
  );
  return (
    <ProjectWorkspaceView
      {...props}
      projection={projection}
      projectWindowPort={projectWindowPort}
      runProjectMutation={runProjectMutation}
    />
  );
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
      exportPipelinePort={exportPipelinePort}
      projection={dirtyProjection}
      projectCorePort={projectCorePortWithApply(async () =>
        dirtyProjection
      )}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    expect(harness.emitCloseRequested()).toBeUndefined();
    expect(
      screen.getByRole("dialog", {
        name: "Salvar alterações antes de fechar?",
      }),
    ).toBeInTheDocument();
  });

  expect(
    screen.getByRole("button", { name: "Salvar e fechar" }),
  ).toBeEnabled();
  expect(
    screen.getByRole("button", { name: "Descartar e fechar" }),
  ).toBeEnabled();
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled();
  expect(
    screen.getByRole("button", { name: "Desfazer", hidden: true }),
  ).toBeDisabled();

  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));

  await waitFor(() => {
    expect(harness.port.resolveClose).toHaveBeenCalledWith("cancel");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  expect(
    await screen.findByRole("button", { name: "Desfazer" }),
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
      exportPipelinePort={exportPipelinePort}
      projection={{
        ...projection,
        state: { ...projection.state, dirty: true },
      }}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Arquivo" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Fechar Projeto" }));
  expect(harness.port.requestClose).toHaveBeenCalledOnce();
  expect(
    await screen.findByRole("dialog", {
      name: "Salvar alterações antes de fechar?",
    }),
  ).toBeInTheDocument();

  fireEvent.click(screen.getByRole("button", { name: "Salvar e fechar" }));
  expect(harness.port.resolveClose).toHaveBeenCalledWith("saveAndClose");
  expect(screen.getByRole("button", { name: "Salvar e fechar" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Descartar e fechar" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeDisabled();
  expect(
    screen.getByRole("button", { name: "Exportar Lâmina", hidden: true }),
  ).toBeDisabled();

  await act(async () => finish());
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    await screen.findByRole("button", { name: "Desfazer" }),
  ).toBeDisabled();
});

test("sends Discard and resumes the unchanged Project after a conclusive save failure", async () => {
  const discardHarness = projectWindowHarness();
  const { unmount } = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={discardHarness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    discardHarness.emitCloseRequested();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Descartar e fechar" }),
  );
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={failureHarness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    failureHarness.emitCloseRequested();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
  fireEvent.click(screen.getByRole("button", { name: "Salvar e fechar" }));

  expect(
    await screen.findByText(
      "O arquivo do Projeto foi alterado fora do MyAlbuns.",
    ),
  ).toBeInTheDocument();
  expect(
    await screen.findByRole("button", { name: "Desfazer" }),
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      projectWindowPort={harness.port}
      onProjectionChange={() => undefined}
    />,
  );
  await waitFor(() => {
    harness.emitCloseRequested();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  fireEvent.click(screen.getByRole("button", { name: "Salvar e fechar" }));

  expect(
    await screen.findByText(
      "Não foi possível confirmar qual revisão ficou no arquivo.",
    ),
  ).toBeInTheDocument();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(
    await screen.findByRole("button", { name: "Desfazer" }),
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
  const controlledExportPipelinePort: ExportPipelinePort = {
    startSheet: (_sheetId, onEvent) => {
      emit = onEvent;
      return {
        completion,
        cancel: async () => "requested",
      };
    },
  };
  const projectCorePort = projectCorePortWithApply(async () => projection);
  projectCorePort.undo = vi.fn(async () => projection);

  render(
    <ProjectWorkspace
      exportPipelinePort={controlledExportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  );
  expect(screen.getByRole("button", { name: "Desfazer" })).toBeDisabled();

  act(() => {
    emit({ event: "started", cancellable: true });
  });
  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  expect(projectCorePort.undo).not.toHaveBeenCalled();

  await act(async () => {
    finish({ status: "cancelled" });
    await completion;
  });

  fireEvent.keyDown(window, { ctrlKey: true, key: "z" });
  expect(projectCorePort.undo).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Fechar" }));
  expect(screen.getByRole("button", { name: "Desfazer" })).toBeEnabled();
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
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
        exportPipelinePort={exportPipelinePort}
        projection={projection}
        projectCorePort={projectCorePortWithApply(async () => projection)}
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

test("uses the documented compact chrome and collapsible contextual sections", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.queryByLabelText("MyAlbuns")).not.toBeInTheDocument();
  expect(screen.queryByText("Álbum Horizonte")).not.toBeInTheDocument();
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
      exportPipelinePort={exportPipelinePort}
      projection={configuredProjection}
      projectCorePort={projectCorePortWithApply(
        async () => configuredProjection,
      )}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.getByText("Dimensão da Lâmina").parentElement).toHaveTextContent(
    "50,8 × 25,4 cm",
  );
  expect(screen.getByText("Dimensão da Página").parentElement).toHaveTextContent(
    "25,4 × 25,4 cm",
  );
  expect(screen.getByText("Resolução").parentElement).toHaveTextContent(
    "240 DPI",
  );
  expect(screen.getByText("Sangria").parentElement).toHaveTextContent(
    "0,25 cm",
  );
  expect(screen.getByText("Área de segurança").parentElement).toHaveTextContent(
    "0,5 cm",
  );
});

test("applies one DPI change and renders the authoritative projection returned by the Project", async () => {
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
  const projectCorePort = projectCorePortWithApply(apply);
  const onProjectionChange = vi.fn();

  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={initialProjection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Design do Álbum" }));
  const input = screen.getByRole("textbox", { name: "DPI" });
  fireEvent.change(input, { target: { value: "600" } });
  expect(apply).not.toHaveBeenCalled();

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Aplicar DPI" }));
    await Promise.resolve();
  });

  expect(apply).toHaveBeenCalledOnce();
  expect(apply).toHaveBeenCalledWith({ kind: "setDpi", dpi: 600 });
  expect(onProjectionChange).toHaveBeenCalledWith(changedProjection);

  view.rerender(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={changedProjection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  expect(screen.getByText("Resolução").parentElement).toHaveTextContent(
    "600 DPI",
  );
  expect(screen.getByRole("textbox", { name: "DPI" })).toHaveValue("600");
  expect(screen.getByRole("button", { name: "Desfazer" })).toBeEnabled();
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
  const save = vi.fn<ProjectCorePort["save"]>(async () => ({
    outcome: {
      kind: "saved",
      revision: savedProjection.state.revision,
    },
    projection: savedProjection,
  }));
  const projectCorePort = projectCorePortWithApply(
    async () => projection,
  );
  projectCorePort.save = save;
  const onProjectionChange = vi.fn();

  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    await Promise.resolve();
  });

  expect(save).toHaveBeenCalledWith(projection.state.revision);
  expect(onProjectionChange).toHaveBeenCalledWith(savedProjection);

  view.rerender(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={savedProjection}
      projectCorePort={projectCorePort}
      onProjectionChange={onProjectionChange}
    />,
  );
  expect(screen.getByRole("button", { name: "Desfazer" })).toBeEnabled();
});

test("uses Ctrl+S for Project save and prevents the browser default", async () => {
  const save = vi.fn<ProjectCorePort["save"]>(async () => ({
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
  const projectCorePort = projectCorePortWithApply(
    async () => projection,
  );
  projectCorePort.save = save;
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
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
  const projectCorePort = projectCorePortWithApply(
    async () => projection,
  );
  projectCorePort.save = vi.fn(async () => {
    throw new SaveProjectError(
      "persisted_baseline_conflict",
      "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
    );
  });
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePort}
      onProjectionChange={() => undefined}
    />,
  );

  await act(async () => {
    fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
    await Promise.resolve();
  });

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "O arquivo do Projeto foi alterado fora do MyAlbuns. O Salvamento não substituiu essas alterações.",
  );
});

test("renders each Grade item from its own composed sheet", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () => twoSheetProjection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 01" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("img", { name: "Prévia da Lâmina 02" }),
  ).toBeInTheDocument();
  expect(screen.getAllByRole("img")).toHaveLength(2);
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
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
      exportPipelinePort={exportPipelinePort}
      projection={decorativeProjection}
      projectCorePort={projectCorePortWithApply(
        async () => decorativeProjection,
      )}
      mediaPreviews={mediaPreviews}
      onProjectionChange={() => undefined}
    />,
  );

  fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));

  expect(screen.getByText("Overlay translúcido.png")).toBeInTheDocument();
  expect(screen.queryByText("Serra ao amanhecer.jpg")).not.toBeInTheDocument();
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

test("renders media usage from the derived Editor projection", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
      onProjectionChange={() => undefined}
    />,
  );

  expect(screen.getByText("1 usos")).toBeInTheDocument();
  expect(screen.getAllByText("0 usos")).toHaveLength(2);
});

test("centers a Grade navigation target in the visible Canvas", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () => twoSheetProjection)}
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
    canvasHarness.props!.continuousCanvasLayout.entries[1].center;
  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(
    1_000 / 2 - targetCenter * 0.5,
  );
});

test("completes Grade navigation requested before Canvas metrics exist", () => {
  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () => twoSheetProjection)}
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

  expect(useEditorView.getState().viewport.offsetX).toBeCloseTo(24);
  expect(useEditorView.getState().focusedSheetId).toBe("sheet-002");
  expect(useEditorView.getState().centeredSheetId).toBe("sheet-002");
});

test("resizes both workspace panels with persistent splitters", () => {
  const firstView = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(async () => projection)}
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
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
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
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
  const oldProjectCorePort = projectCorePortWithApply(oldApply);
  const newProjectCorePort = projectCorePortWithApply(newApply);
  useEditorView.setState({ selectedFrameId: "frame-001" });

  const view = render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={oldProjectCorePort}
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
      exportPipelinePort={exportPipelinePort}
      projection={otherProject}
      projectCorePort={newProjectCorePort}
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
      exportPipelinePort={exportPipelinePort}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(apply)}
      onProjectionChange={() => undefined}
    />,
  );

  act(() => {
    canvasHarness.props?.onCenteredSheetChange?.("sheet-002");
  });
  fireEvent.doubleClick(screen.getByText("Campo.jpg").closest("button")!);

  expect(apply).toHaveBeenCalledWith({
    kind: "fillLeftmostPlaceholder",
    sheetId: "sheet-002",
    mediaId: "media-002",
  });
});

test("starts Exportação for the Canvas-centered Lâmina even while focus remains on another Lâmina", () => {
  const startSheet = vi.fn<ExportPipelinePort["startSheet"]>(() => ({
    completion: Promise.resolve({
      status: "completed",
      result: { widthPx: 600, heightPx: 300 },
    }),
    cancel: async () => "not_found",
  }));

  render(
    <ProjectWorkspace
      exportPipelinePort={{ startSheet }}
      projection={twoSheetProjection}
      projectCorePort={projectCorePortWithApply(async () =>
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

  expect(startSheet).toHaveBeenCalledWith(
    {
      projectName: "Álbum Horizonte",
      sheetId: "sheet-002",
      sheetNumber: 2,
    },
    expect.any(Function),
  );
});

test("forwards simultaneous Canvas Pan and Zoom as one intent", () => {
  const apply = vi.fn(async () => projection);

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
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
    .fn<ProjectCorePort["apply"]>()
    .mockImplementationOnce(() => first.promise)
    .mockImplementationOnce(() => second.promise);
  const onProjectionChange = vi.fn();

  render(
    <ProjectWorkspace
      exportPipelinePort={exportPipelinePort}
      projection={projection}
      projectCorePort={projectCorePortWithApply(apply)}
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
