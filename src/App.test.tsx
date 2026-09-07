import { useEffect, useState, type ComponentProps } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import AppView from "./App";
import {
  type LogEvent,
  type Logger,
  silentLogger,
} from "./application/logging";
import type {
  ExportAttempt,
  ExportPipelinePort,
  ExportProgressEvent,
  MediaPreview,
  MediaPreviewPort,
  MediaPreviewRequest,
  ProjectStartupPort,
  ProjectCorePort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type {
  ProjectDialogAction,
  ProjectDialogPort,
} from "./application/projectDialogPort";
import {
  createWorkspacePreferences,
  type WorkspacePreferences,
  type WorkspacePreferencesPort,
} from "./application/workspacePreferences";
import { MediaPreviewError } from "./application/projectPorts";
import type { EditorProjection } from "./domain/project";
import {
  createEmptyProjection,
  representativeProjection,
} from "./test/projectFixtures";

vi.mock("./components/AlbumCanvas", () => ({
  AlbumCanvas: ({
    onMediaDemandChange,
    mediaPreviewUrls,
    onGraphicsUnavailable,
    composition,
  }: {
    onMediaDemandChange?: (demand: {
      visibleMediaIds: readonly string[];
      preloadMediaIds: readonly string[];
    }) => void;
    mediaPreviewUrls?: Readonly<Record<string, string>>;
    composition?: EditorProjection["composition"];
    onGraphicsUnavailable?: (diagnostic: {
      supported: false;
      code: "webgl2_unavailable";
      renderer: string;
      reason: string;
      limits: null;
    }) => void;
  }) => {
    const [demandReported, setDemandReported] = useState(false);
    useEffect(() => {
      onMediaDemandChange?.({
        visibleMediaIds: ["media-001"],
        preloadMediaIds: [],
      });
      setDemandReported(true);
    }, [onMediaDemandChange]);
    return (
      <>
        <div
          data-testid="album-canvas"
          data-demand-reported={demandReported}
          data-media-preview={mediaPreviewUrls?.["media-001"] ?? ""}
          data-photo-draw-width={
            composition?.sheets[0]?.frames[0]?.photo?.drawRect.width ?? ""
          }
        />
        <button
          type="button"
          aria-label="Esvaziar demanda de Canvas"
          onClick={() =>
            onMediaDemandChange?.({
              visibleMediaIds: [],
              preloadMediaIds: [],
            })
          }
        />
        <button
          type="button"
          aria-label="Simular perda grafica"
          onClick={() =>
            onGraphicsUnavailable?.({
              supported: false,
              code: "webgl2_unavailable",
              renderer: "indisponivel",
              reason: "WebGL2 runtime failure.",
              limits: null,
            })
          }
        />
      </>
    );
  },
}));

const projection = createEmptyProjection();

const projectCorePort: ProjectCorePort = {
  load: async () => projection,
  validateAlbumInformation: async () => ({
    errors: [],
    impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
  }),
  apply: async () => projection,
  applyWithOutcome: async () => ({
    projection,
    affectedFrameId: null,
    affectedSheetId: null,
  }),
  importPhoto: async () => ({ kind: "cancelled", projection }),
  readFrameDragThreshold: async () => ({ x: 5, y: 5 }),
  previewFrameGeometry: async () => { throw new Error("Frame geometry preview is not configured in this fixture."); },
  resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
  relink: async () => projection,
  undo: async () => projection,
  redo: async () => projection,
  save: async () => {
    throw new Error("Salvamento não configurado neste teste.");
  },
  saveAs: async () => {
    throw new Error("Salvar como não configurado neste teste.");
  },
};
const mediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: async () => null,
  retryUnavailableMedia: async (mediaId) => ({
    mediaId,
    state: "unavailable",
    url: null,
  }),
  onMediaChanged: async () => () => undefined,
  onCacheProcessorWarning: async () => () => undefined,
};
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
interface LegacyExportPort {
  startSheet(
    sheetId: string,
    onEvent: (event: ExportProgressEvent) => void,
  ): ExportAttempt;
}
const exportPort: LegacyExportPort = {
  startSheet: (sheetId, onEvent) =>
    exportPipelinePort.startSheet(
      { projectName: "Projeto de teste", sheetId, sheetNumber: 1 },
      onEvent,
    ),
};
const projectSessionPort = projectCorePort;
const projectWindowPort: ProjectWindowPort = {
  onCloseRequested: async () => () => undefined,
  requestClose: async () => ({ kind: "closed" }),
  resolveClose: async () => ({ kind: "closed" }),
};
const projectDialogPort: ProjectDialogPort = {
  acquire: () => ({
    dismiss: async () => undefined,
    present: async () => undefined,
  }),
};
const projectStartupPort: ProjectStartupPort = {
  confirmUiReady: async () => undefined,
};
const canvasGraphicsDiagnosticProbe = () =>
  ({
    supported: true,
    renderer: "NVIDIA GeForce RTX",
    reason: "WebGL2 acelerado por hardware confirmado.",
    limits: {
      maxTextureSizePx: 16_384,
      maxRenderbufferSizePx: 16_384,
      maxTextureImageUnits: 16,
    },
  }) as const;

type TestAppProps = Omit<
  ComponentProps<typeof AppView>,
  | "exportPipelinePort"
  | "projectCorePort"
  | "projectDialogPort"
  | "projectStartupPort"
  | "workspacePreferencesMode"
  | "workspacePreferencesPort"
> & {
  exportPipelinePort?: ExportPipelinePort;
  exportPort?: LegacyExportPort;
  projectCorePort?: ProjectCorePort;
  projectSessionPort?: ProjectCorePort;
  projectDialogPort?: ProjectDialogPort;
  projectStartupPort: Partial<ProjectStartupPort> &
    Pick<ProjectStartupPort, "confirmUiReady">;
  workspacePreferencesMode?: "memory";
  workspacePreferencesPort?: WorkspacePreferencesPort;
};

function projectDialogHarness() {
  const dismiss = vi.fn(async () => undefined);
  const present = vi.fn(async () => undefined);
  let listener: (action: ProjectDialogAction) => void = () => undefined;
  return {
    dismiss,
    emit: (action: ProjectDialogAction) => listener(action),
    present,
    port: {
      acquire: (nextListener) => {
        listener = nextListener;
        return { dismiss, present };
      },
    } satisfies ProjectDialogPort,
  };
}

function App({
  exportPipelinePort: providedExportPipelinePort,
  exportPort: providedLegacyExportPort,
  projectCorePort: providedProjectCorePort,
  projectSessionPort,
  projectDialogPort: providedProjectDialogPort = projectDialogPort,
  projectStartupPort: providedProjectStartupPort,
  workspacePreferencesMode,
  workspacePreferencesPort,
  ...props
}: TestAppProps) {
  const effectiveExportPipelinePort =
    providedExportPipelinePort ??
    (providedLegacyExportPort
      ? {
          startSheet: (selection, onEvent) =>
            providedLegacyExportPort.startSheet(selection.sheetId, onEvent),
        }
      : exportPipelinePort);
  const sharedProps = {
    ...props,
    exportPipelinePort: effectiveExportPipelinePort,
    projectCorePort:
      providedProjectCorePort ?? projectSessionPort ?? projectCorePort,
    projectDialogPort: providedProjectDialogPort,
    projectStartupPort: {
      ...projectStartupPort,
      ...providedProjectStartupPort,
    },
  };
  return workspacePreferencesPort ? (
    <AppView {...sharedProps} workspacePreferencesPort={workspacePreferencesPort} />
  ) : (
    <AppView
      {...sharedProps}
      workspacePreferencesMode={workspacePreferencesMode ?? "memory"}
    />
  );
}

test("keeps the Recovery decision out of the Project WebView startup", async () => {
  const load = vi.fn(async () => projection);

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      mediaPreviewPort={mediaPreviewPort}
      projectStartupPort={projectStartupPort}
      projectCorePort={{ ...projectCorePort, load }}
      projectWindowPort={projectWindowPort}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      logger={silentLogger}
    />,
  );

  expect(
    await screen.findByRole("button", { name: "Exportar Lâmina" }),
  ).toBeInTheDocument();
  expect(load).toHaveBeenCalledOnce();
  expect(
    screen.queryByRole("dialog", { name: "Recuperar trabalho não salvo?" }),
  ).not.toBeInTheDocument();
  expect(
    document.querySelector("[data-project-owner-surface]"),
  ).not.toBeInTheDocument();
});




test("surfaces the durable Save As terminal when the previous WebView is restored", async () => {
  const dialog = projectDialogHarness();
  window.location.hash = "#save-as-state-indeterminate";
  try {
    render(
      <App
        exportPipelinePort={exportPipelinePort}
        mediaPreviewPort={mediaPreviewPort}
        projectStartupPort={projectStartupPort}
        projectCorePort={projectCorePort}
        projectDialogPort={dialog.port}
        projectWindowPort={projectWindowPort}
        graphicsProbe={canvasGraphicsDiagnosticProbe}
        canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
        logger={silentLogger}
      />,
    );

    await waitFor(() =>
      expect(dialog.present).toHaveBeenCalledWith({
        kind: "projectOperationFailure",
        message:
          "Não foi possível confirmar o destino de Salvar como. A Sessão anterior foi mantida; reinspecione o destino antes de reutilizá-lo.",
      }),
    );
  } finally {
    window.location.hash = "";
  }
});

test("reports a defensive Project Canvas failure without claiming that no Session exists", async () => {
  const load = vi.fn(async () => projection);
  const prepareMediaPreviews = vi.fn(async () => null);
  const dialog = projectDialogHarness();
  const requestClose = vi.fn(async () => ({ kind: "closed" as const }));
  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={dialog.port}
      projectWindowPort={{ ...projectWindowPort, requestClose }}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{ ...projectCorePort, load }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: false,
        code: "webgl2_unavailable",
        renderer: "indisponível",
        reason: "WebGL2 acelerado por hardware não foi confirmado.",
        limits: null,
      })}
    />,
  );

  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "graphicsFailure",
      reason: "WebGL2 acelerado por hardware não foi confirmado.",
    }),
  );
  expect(
    screen.queryByRole("heading", {
      name: "O Canvas não pôde ser iniciado",
    }),
  ).not.toBeInTheDocument();

  expect(load).not.toHaveBeenCalled();
  expect(prepareMediaPreviews).not.toHaveBeenCalled();

  act(() => dialog.emit("closeProjectAfterGraphicsFailure"));
  await waitFor(() => expect(dialog.dismiss).toHaveBeenCalledOnce());
  expect(requestClose).toHaveBeenCalledOnce();
});

test("opens the Project in the real workspace when hardware WebGL2 is available", async () => {
  const logEvents: LogEvent[] = [];
  const logger: Logger = {
    write: (event) => logEvents.push(event),
  };
  const load = vi.fn(async (_operationId: string) => projection);
  let mediaChangedSubscribed = false;
  const confirmUiReady = vi.fn(async () => {
    expect(mediaChangedSubscribed).toBe(true);
  });
  render(
    <App
      workspacePreferencesMode="memory"
      projectDialogPort={projectDialogPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={{ ...projectStartupPort, confirmUiReady }}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        onMediaChanged: async () => {
          mediaChangedSubscribed = true;
          return () => undefined;
        },
      }}
      projectCorePort={{ ...projectCorePort, load }}
      logger={logger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  expect(
    await screen.findByRole("button", { name: "Exportar Lâmina" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("menubar", { name: "Menu principal" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(confirmUiReady).toHaveBeenCalledOnce());
  expect(
    screen.getByText("Álbum Horizonte", {
      selector: ".ui-application-header__identity strong",
    }),
  ).toBeInTheDocument();
  expect(screen.queryByText("NVIDIA GeForce RTX")).not.toBeInTheDocument();
  expect(logEvents).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        component: "application",
        event: "project_load_completed",
        projectId: projection.state.projectId,
        sheetCount: projection.composition.sheets.length,
      }),
      expect.objectContaining({
        component: "graphics",
        event: "graphics_probe_succeeded",
      }),
    ]),
  );
  const loadStarted = logEvents.find(
    ({ event }) => event === "project_load_started",
  );
  expect(load).toHaveBeenCalledWith(loadStarted?.operationId);
});

test("confirms Project UI readiness only after shared preferences hydrate", async () => {
  let finishPreferenceLoad: (value: WorkspacePreferences) => void =
    () => undefined;
  const preferenceLoad = new Promise<WorkspacePreferences>((resolve) => {
    finishPreferenceLoad = resolve;
  });
  const confirmUiReady = vi.fn(async () => undefined);

  render(
    <App
      exportPort={exportPort}
      projectStartupPort={{ confirmUiReady }}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={mediaPreviewPort}
      projectSessionPort={projectSessionPort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
      workspacePreferencesPort={{
        load: () => preferenceLoad,
        update: async () => createWorkspacePreferences(),
      }}
    />,
  );

  await screen.findByRole("button", { name: "Exportar Lâmina" });
  expect(confirmUiReady).not.toHaveBeenCalled();

  act(() => finishPreferenceLoad(createWorkspacePreferences()));
  await waitFor(() => expect(confirmUiReady).toHaveBeenCalledOnce());
});

test("synchronizes a no-cache reopen while Monitor startup remains pending without adding History", async () => {
  let notifyMediaChanged: ((mediaIds: readonly string[]) => void) | undefined;
  let completeUiReady: (() => void) | undefined;
  const refreshedProjection: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      album: {
        ...representativeProjection.state.album,
        media: representativeProjection.state.album.media.map((media) =>
          media.id === "media-001"
            ? { ...media, name: "Foto confirmada sem Cache.jpg" }
            : media,
        ),
      },
    },
    composition: {
      ...representativeProjection.composition,
      sheets: representativeProjection.composition.sheets.map((sheet) => ({
        ...sheet,
        frames: sheet.frames.map((frame) =>
          frame.photo?.mediaId === "media-001"
            ? {
                ...frame,
                photo: {
                  ...frame.photo,
                  name: "Foto confirmada sem Cache.jpg",
                  drawRect: { ...frame.photo.drawRect, width: 123_000 },
                },
              }
            : frame,
        ),
      })),
    },
  };
  const load = vi
    .fn()
    .mockResolvedValueOnce(representativeProjection)
    .mockResolvedValue(refreshedProjection);
  const apply = vi.fn(async () => representativeProjection);
  const applyWithOutcome = vi.fn(async () => ({
    projection: representativeProjection,
    affectedFrameId: null,
    affectedSheetId: null,
  }));
  const undo = vi.fn(async () => representativeProjection);
  const redo = vi.fn(async () => representativeProjection);
  const confirmUiReady = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        expect(notifyMediaChanged).toBeTypeOf("function");
        completeUiReady = resolve;
        notifyMediaChanged?.(["media-001"]);
      }),
  );

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={{ ...projectStartupPort, confirmUiReady }}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews: async () => [
          {
            mediaId: "media-001",
            state: "cache_unavailable" as const,
            url: null,
          },
        ],
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
      }}
      projectCorePort={{
        ...projectCorePort,
        load,
        apply,
        applyWithOutcome,
        undo,
        redo,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
    />,
  );

  await waitFor(() => expect(confirmUiReady).toHaveBeenCalledOnce());
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-photo-draw-width",
    "123000",
  );
  expect(
    screen.getByRole("button", {
      name: /^Foto confirmada sem Cache\.jpg(?:\.|$)/,
    }),
  ).toBeInTheDocument();
  expect(refreshedProjection.state.revision).toBe(
    representativeProjection.state.revision,
  );
  expect(apply).not.toHaveBeenCalled();
  expect(applyWithOutcome).not.toHaveBeenCalled();
  expect(undo).not.toHaveBeenCalled();
  expect(redo).not.toHaveBeenCalled();
  act(() => completeUiReady?.());
});

test("prepares real media previews after opening without blocking the Workspace", async () => {
  const logEvents: LogEvent[] = [];
  const logger: Logger = {
    write: (event) => logEvents.push(event),
  };
  const prepareMediaPreviews = vi.fn(async () => [
      {
        mediaId: "media-001",
        state: "ready" as const,
        url: "http://myalbuns-cache.localhost/opaque-media-token",
      },
    ]);

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={projectCorePort}
      logger={logger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  expect(
    await screen.findByRole("button", { name: "Exportar Lâmina" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  expect(logEvents).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        component: "media-preview",
        event: "media_preview_completed",
        projectId: projection.state.projectId,
      }),
    ]),
  );
});

test("shows a ready preview while the remaining previews are still being prepared", async () => {
  let publish: ((preview: MediaPreview) => void) | undefined;
  let finish!: (previews: readonly MediaPreview[]) => void;
  const prepareMediaPreviews = vi.fn((_demand: MediaPreviewRequest, onPreview: (preview: MediaPreview) => void) => {
    publish = onPreview;
    return new Promise<readonly MediaPreview[]>((resolve) => { finish = resolve; });
  });
  const preview: MediaPreview = {
    mediaId: "media-001",
    state: "ready",
    url: "http://myalbuns-cache.localhost/first-ready",
  };
  render(
    <App
      mediaPreviewPort={{ ...mediaPreviewPort, prepareMediaPreviews }}
      projectCorePort={{ ...projectCorePort, load: async () => representativeProjection }}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
    />,
  );
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalled());

  act(() => publish?.(preview));

  expect(document.querySelector('[data-media-id="media-001"] img')).toHaveAttribute("src", preview.url);
  expect(screen.getByTestId("album-canvas")).toHaveAttribute("data-media-preview", preview.url);
  await act(async () => finish([preview]));
});

test("shows warm Fotos immediately after returning from Decorativos while the next demand is pending", async () => {
  vi.stubGlobal("IntersectionObserver", class {
    constructor(private callback: IntersectionObserverCallback) {}
    observe(target: HTMLElement) {
      this.callback([{ target, isIntersecting: true } as unknown as IntersectionObserverEntry], this as unknown as IntersectionObserver);
    }
    disconnect() {}
  });
  const requests: {
    demand: MediaPreviewRequest;
    publish: (preview: MediaPreview) => void;
    finish: (previews: readonly MediaPreview[]) => void;
  }[] = [];
  const prepareMediaPreviews: MediaPreviewPort["prepareMediaPreviews"] = (demand, publish) =>
    new Promise((finish) => { requests.push({ demand, publish, finish }); });
  const preview: MediaPreview = { mediaId: "media-002", state: "ready", url: "http://myalbuns-cache.localhost/warm-photo" };
  const view = render(
    <App
      mediaPreviewPort={{ ...mediaPreviewPort, prepareMediaPreviews }}
      projectCorePort={{ ...projectCorePort, load: async () => representativeProjection }}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
    />,
  );
  try {
    await waitFor(() => expect(requests[requests.length - 1]?.demand.visibleMediaIds).toContain("media-002"));
    act(() => requests[requests.length - 1].publish(preview));
    expect(document.querySelector('[data-media-id="media-002"] img')).toHaveAttribute("src", preview.url);
    fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
    await waitFor(() => expect(requests[requests.length - 1]?.demand.preloadMediaIds).toContain("media-002"));
    await act(async () => requests[requests.length - 1].finish([preview]));
    fireEvent.click(screen.getByRole("button", { name: "Fotos" }));
    expect(document.querySelector('[data-media-id="media-002"] img')).toHaveAttribute("src", preview.url);
  } finally {
    view.unmount();
    vi.unstubAllGlobals();
  }
});

test("shows previously loaded Panel photos immediately after scrolling away and back", async () => {
  const observers: { callback: IntersectionObserverCallback; targets: HTMLElement[] }[] = [];
  vi.stubGlobal("IntersectionObserver", class {
    targets: HTMLElement[] = [];
    constructor(public callback: IntersectionObserverCallback) { observers.push(this); }
    observe(target: HTMLElement) { this.targets.push(target); }
    disconnect() {}
  });
  const requests: {
    demand: MediaPreviewRequest;
    finish: (previews: readonly MediaPreview[]) => void;
  }[] = [];
  const prepareMediaPreviews: MediaPreviewPort["prepareMediaPreviews"] = (demand) =>
    new Promise((finish) => { requests.push({ demand, finish }); });
  const top: MediaPreview = {
    mediaId: "media-002", state: "ready", url: "http://myalbuns-cache.localhost/top-photo",
  };
  const bottom: MediaPreview = {
    mediaId: "media-003", state: "ready", url: "http://myalbuns-cache.localhost/bottom-photo",
  };
  const view = render(
    <App
      mediaPreviewPort={{ ...mediaPreviewPort, prepareMediaPreviews }}
      projectCorePort={{ ...projectCorePort, load: async () => representativeProjection }}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
    />,
  );
  const scrollTo = (mediaId: string) => act(() => {
    for (const observer of observers) {
      observer.callback(observer.targets.map((target) => ({
        target, isIntersecting: target.dataset.mediaId === mediaId,
      }) as unknown as IntersectionObserverEntry), observer as unknown as IntersectionObserver);
    }
  });
  try {
    await screen.findByRole("group", { name: "Grade de Fotos" });
    scrollTo(top.mediaId);
    await waitFor(() => expect(requests[requests.length - 1]?.demand.visibleMediaIds).toContain(top.mediaId));
    await act(async () => requests[requests.length - 1].finish([top]));
    expect(document.querySelector('[data-media-id="media-002"] img')).toHaveAttribute("src", top.url);
    const loadedImage = document.querySelector('[data-media-id="media-002"] img');

    scrollTo(bottom.mediaId);
    await waitFor(() => expect(requests[requests.length - 1]?.demand.visibleMediaIds).toContain(bottom.mediaId));
    // The native completion includes its bounded set of recent residents.
    await act(async () => requests[requests.length - 1].finish([top, bottom]));
    scrollTo(top.mediaId);

    // No new native response has arrived: a previously shown thumbnail must
    // already be drawable on returning to its row.
    expect(document.querySelector('[data-media-id="media-002"] img')).toHaveAttribute("src", top.url);
    expect(document.querySelector('[data-media-id="media-002"] img')).toBe(loadedImage);
  } finally {
    view.unmount();
    vi.unstubAllGlobals();
  }
});

test("ignores preview events and completion from a demand superseded by a Monitor refresh", async () => {
  const requests: {
    publish: (preview: MediaPreview) => void;
    finish: (previews: readonly MediaPreview[]) => void;
  }[] = [];
  let notifyMediaChanged!: (mediaIds: readonly string[]) => void;
  const prepareMediaPreviews: MediaPreviewPort["prepareMediaPreviews"] = (_demand, publish) =>
    new Promise((finish) => { requests.push({ publish, finish }); });
  const preview: MediaPreview = { mediaId: "media-001", state: "ready", url: "http://myalbuns-cache.localhost/new" };
  render(
    <App
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
      }}
      projectCorePort={{ ...projectCorePort, load: async () => representativeProjection }}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
    />,
  );
  await waitFor(() => expect(requests).toHaveLength(1));
  act(() => notifyMediaChanged(["media-001"]));
  await waitFor(() => expect(requests).toHaveLength(2));
  act(() => requests[1].publish(preview));
  const stale = { ...preview, url: "http://myalbuns-cache.localhost/old" };
  await act(async () => {
    requests[0].publish(stale);
    requests[0].finish([stale]);
  });
  expect(document.querySelector('[data-media-id="media-001"] img')).toHaveAttribute("src", preview.url);
  await act(async () => requests[1].finish([preview]));
  act(() => requests[1].publish(stale));
  expect(document.querySelector('[data-media-id="media-001"] img')).toHaveAttribute("src", preview.url);
});

test("reprepares demanded media when the stable Monitor reports a change", async () => {
  let notifyMediaChanged: ((mediaIds: readonly string[]) => void) | undefined;
  const prepareMediaPreviews = vi
    .fn()
    .mockResolvedValueOnce([
      {
        mediaId: "media-001",
        state: "ready" as const,
        url: "http://myalbuns-cache.localhost/generation-one",
      },
    ])
    .mockResolvedValueOnce([
      {
        mediaId: "media-001",
        state: "ready" as const,
        url: "http://myalbuns-cache.localhost/generation-two",
      },
    ]);

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    "http://myalbuns-cache.localhost/generation-one",
  );

  act(() => notifyMediaChanged?.(["media-001"]));

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    "http://myalbuns-cache.localhost/generation-two",
  );
});

test("keeps the last known preview when linked media becomes unavailable", async () => {
  let notifyMediaChanged: ((mediaIds: readonly string[]) => void) | undefined;
  const retainedUrl = "http://myalbuns-cache.localhost/generation-one";
  const prepareMediaPreviews = vi
    .fn()
    .mockResolvedValueOnce([
      { mediaId: "media-001", state: "ready" as const, url: retainedUrl },
    ])
    .mockResolvedValueOnce([
      {
        mediaId: "media-001",
        state: "unavailable" as const,
        url: retainedUrl,
      },
    ]);

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    retainedUrl,
  );

  act(() => notifyMediaChanged?.(["media-001"]));

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    retainedUrl,
  );
  expect(
    screen.getByRole("status", { name: "Indisponível · prévia anterior" }),
  ).toBeInTheDocument();
});

test("keeps the last representation only as visual context when the Original is absent", async () => {
  const retainedUrl = "http://myalbuns-cache.localhost/generation-one";
  const prepareMediaPreviews = vi.fn().mockResolvedValue([
    {
      mediaId: "media-001",
      state: "absent" as const,
      url: retainedUrl,
    },
  ]);

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce(), {
    timeout: 5_000,
  });
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    retainedUrl,
  );
  expect(
    screen.getByRole("status", { name: "Arquivo ausente · prévia anterior" }),
  ).toBeInTheDocument();
});

test("shows the canonical Project warning when repeated processor failures suspend Cache", async () => {
  let warnCacheSuspended:
    | ((warning: { state: "suspended"; message: string }) => void)
    | undefined;
  const dialog = projectDialogHarness();
  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      projectDialogPort={dialog.port}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews: async () => [],
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async (listener) => {
          warnCacheSuspended = listener;
          return () => undefined;
        },
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await screen.findByRole("button", { name: "Exportar Lâmina" });
  act(() =>
    warnCacheSuspended?.({
      state: "suspended",
      message:
        "O Cache foi suspenso após falhas repetidas do Processador de Imagens.",
    }),
  );

  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "projectOperationFailure",
      message:
        "O Cache foi suspenso após falhas repetidas do Processador de Imagens.",
    }),
  );
  expect(screen.getByRole("button", { name: "Exportar Lâmina" })).toBeEnabled();
  expect(screen.getByTestId("album-canvas")).toBeInTheDocument();
});

test("registers the Cache warning listener before the first preview demand", async () => {
  let resolveWarningRegistration:
    | ((dispose: () => void) => void)
    | undefined;
  let warnCacheSuspended:
    | ((warning: { state: "suspended"; message: string }) => void)
    | undefined;
  const prepareMediaPreviews = vi.fn(async () => {
    warnCacheSuspended?.({
      state: "suspended",
      message:
        "O Cache foi suspenso após falhas repetidas do Processador de Imagens.",
    });
    return [];
  });

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: (listener) =>
          new Promise<() => void>((resolve) => {
            resolveWarningRegistration = (dispose) => {
              warnCacheSuspended = listener;
              resolve(dispose);
            };
          }),
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await screen.findByRole("button", { name: "Exportar Lâmina" });
  await waitFor(() =>
    expect(screen.getByTestId("album-canvas")).toHaveAttribute(
      "data-demand-reported",
      "true",
    ),
  );
  await act(async () => {
    await Promise.resolve();
  });
  expect(prepareMediaPreviews).not.toHaveBeenCalled();

  act(() => resolveWarningRegistration?.(() => undefined));

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
});

test("registers the media-change listener before the first preview demand", async () => {
  let resolveMediaRegistration:
    | ((dispose: () => void) => void)
    | undefined;
  let notifyMediaChanged: ((mediaIds: readonly string[]) => void) | undefined;
  const prepareMediaPreviews = vi.fn(async () => []);

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: (listener) =>
          new Promise<() => void>((resolve) => {
            resolveMediaRegistration = (dispose) => {
              notifyMediaChanged = listener;
              resolve(dispose);
            };
          }),
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await screen.findByRole("button", { name: "Exportar Lâmina" });
  await waitFor(() =>
    expect(screen.getByTestId("album-canvas")).toHaveAttribute(
      "data-demand-reported",
      "true",
    ),
  );
  await act(async () => {
    await Promise.resolve();
  });
  expect(prepareMediaPreviews).not.toHaveBeenCalled();

  act(() => resolveMediaRegistration?.(() => undefined));

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  act(() => notifyMediaChanged?.(["media-001"]));
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
});

test("keeps the newest media projection when equal-revision refreshes resolve out of order", async () => {
  let notifyMediaChanged: ((mediaIds: readonly string[]) => void) | undefined;
  let resolveOlder!: (projection: typeof representativeProjection) => void;
  let resolveNewer!: (projection: typeof representativeProjection) => void;
  const projectionNamed = (name: string) => ({
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      album: {
        ...representativeProjection.state.album,
        media: representativeProjection.state.album.media.map((media, index) =>
          index === 0 ? { ...media, name } : media,
        ),
      },
    },
  });
  const load = vi
    .fn()
    .mockResolvedValueOnce(representativeProjection)
    .mockImplementationOnce(
      () =>
        new Promise<typeof representativeProjection>((resolve) => {
          resolveOlder = resolve;
        }),
    )
    .mockImplementationOnce(
      () =>
        new Promise<typeof representativeProjection>((resolve) => {
          resolveNewer = resolve;
        }),
    );

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
      }}
      projectCorePort={{ ...projectCorePort, load }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await screen.findByRole("button", { name: /Serra ao amanhecer\.jpg/i });
  act(() => {
    notifyMediaChanged?.(["media-001"]);
  });
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  act(() => {
    notifyMediaChanged?.(["media-001"]);
  });
  await waitFor(() => expect(load).toHaveBeenCalledTimes(3));
  await act(async () => {
    resolveNewer(projectionNamed("Observação nova.jpg"));
  });
  await screen.findByRole("button", {
    name: /^Observação nova\.jpg(?:\.|$)/,
  });

  await act(async () => {
    resolveOlder(projectionNamed("Observação antiga.jpg"));
  });
  expect(
    screen.getByRole("button", { name: /^Observação nova\.jpg(?:\.|$)/ }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", {
      name: /^Observação antiga\.jpg(?:\.|$)/,
    }),
  ).not.toBeInTheDocument();
});

test("keeps recovery actions hidden until the first authoritative media observation", async () => {
  const prepareMediaPreviews = vi.fn().mockResolvedValue([]);

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  expect(
    screen.queryByRole("status", {
      name: /^(Arquivo ausente|Indisponível|Prévia indisponível)/,
    }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Tentar novamente o arquivo de/i }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Religar arquivo de/i }),
  ).not.toBeInTheDocument();
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    "",
  );
});

test("retries an unavailable occurrence explicitly and refreshes it without Relink", async () => {
  const recoveredUrl = "asset://localhost/cache/media-001-recovered.jpg";
  let notifyMediaChanged: ((mediaIds: readonly string[]) => void) | undefined;
  const refreshedProjection: EditorProjection = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      album: {
        ...representativeProjection.state.album,
        media: representativeProjection.state.album.media.map((media) =>
          media.id === "media-001"
            ? { ...media, sourceWidthPx: 23, sourceHeightPx: 5 }
            : media,
        ),
      },
    },
    composition: {
      ...representativeProjection.composition,
      sheets: [
        {
          ...representativeProjection.composition.sheets[0],
          frames: [
            {
              ...representativeProjection.composition.sheets[0].frames[0],
              photo: {
                ...representativeProjection.composition.sheets[0].frames[0]
                  .photo!,
                drawRect: {
                  ...representativeProjection.composition.sheets[0].frames[0]
                    .photo!.drawRect,
                  width: 123_000,
                },
              },
            },
          ],
        },
      ],
    },
  };
  const prepareMediaPreviews = vi
    .fn()
    .mockResolvedValueOnce([
      { mediaId: "media-001", state: "unavailable" as const, url: null },
    ])
    .mockResolvedValueOnce([
      { mediaId: "media-001", state: "ready" as const, url: recoveredUrl },
    ]);
  const retryUnavailableMedia = vi.fn(async () => {
    notifyMediaChanged?.(["media-001"]);
    return {
      mediaId: "media-001",
      state: "ready" as const,
      url: null,
    };
  });
  const load = vi
    .fn()
    .mockResolvedValueOnce(representativeProjection)
    .mockResolvedValue(refreshedProjection);
  const relink = vi.fn(async () => representativeProjection);
  const apply = vi.fn(async () => representativeProjection);

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        retryUnavailableMedia,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load,
        apply,
        relink,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  const retry = await screen.findByRole("button", {
    name: /Tentar novamente o arquivo de/i,
  });
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-photo-draw-width",
    "400000",
  );
  fireEvent.click(retry);

  await waitFor(() =>
    expect(retryUnavailableMedia).toHaveBeenCalledWith("media-001", expect.any(Function)),
  );
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(load).toHaveBeenCalledTimes(3));
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    recoveredUrl,
  );
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-photo-draw-width",
    "123000",
  );
  expect(refreshedProjection.state.revision).toBe(
    representativeProjection.state.revision,
  );
  expect(relink).not.toHaveBeenCalled();
  expect(apply).not.toHaveBeenCalled();
});

test("keeps retry actionable after an unavailable-media IPC failure without mutating Project", async () => {
  const logEvents: LogEvent[] = [];
  const logger: Logger = { write: (event) => logEvents.push(event) };
  let resolveInitialPreview!: (previews: readonly MediaPreview[]) => void;
  const prepareMediaPreviews = vi.fn(
    () =>
      new Promise<readonly MediaPreview[]>((resolve) => {
        resolveInitialPreview = resolve;
      }),
  );
  const retryUnavailableMedia = vi.fn(async () => {
    throw new MediaPreviewError(
      "read_failed",
      "A raiz continua temporariamente indisponível.",
    );
  });
  const relink = vi.fn(async () => representativeProjection);
  const apply = vi.fn(async () => representativeProjection);

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        retryUnavailableMedia,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
        apply,
        relink,
      }}
      logger={logger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  await act(async () => {
    resolveInitialPreview([
      { mediaId: "media-001", state: "unavailable", url: null },
    ]);
  });
  fireEvent.click(
    screen.getByRole("button", {
      name: /Tentar novamente o arquivo de/i,
    }),
  );

  await waitFor(() =>
    expect(logEvents).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          event: "media_retry_failed",
          reason: "read_failed",
        }),
      ]),
    ),
  );
  expect(prepareMediaPreviews).toHaveBeenCalledOnce();
  expect(
    screen.getByRole("button", { name: /Tentar novamente o arquivo de/i }),
  ).toBeEnabled();
  expect(screen.getByRole("status", { name: "Indisponível" })).toBeInTheDocument();
  expect(relink).not.toHaveBeenCalled();
  expect(apply).not.toHaveBeenCalled();
});

test("replaces unavailable retry with a cache-only failure after authoritative refresh", async () => {
  let notifyMediaChanged: ((mediaIds: readonly string[]) => void) | undefined;
  const prepareMediaPreviews = vi
    .fn()
    .mockResolvedValueOnce([
      { mediaId: "media-001", state: "unavailable" as const, url: null },
    ])
    .mockResolvedValueOnce([
      {
        mediaId: "media-001",
        state: "cache_unavailable" as const,
        url: null,
      },
    ]);
  const retryUnavailableMedia = vi.fn();
  const relink = vi.fn(async () => representativeProjection);
  const apply = vi.fn(async () => representativeProjection);

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        retryUnavailableMedia,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={{
        ...projectCorePort,
        load: async () => representativeProjection,
        apply,
        relink,
      }}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  expect(
    await screen.findByRole("button", {
      name: /Tentar novamente o arquivo de/i,
    }),
  ).toBeEnabled();

  act(() => notifyMediaChanged?.(["media-001"]));

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
  expect(
    screen.getByRole("status", { name: "Prévia indisponível" }),
  ).toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Tentar novamente o arquivo de/i }),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByRole("button", { name: /Religar arquivo de/i }),
  ).not.toBeInTheDocument();
  expect(retryUnavailableMedia).not.toHaveBeenCalled();
  expect(relink).not.toHaveBeenCalled();
  expect(apply).not.toHaveBeenCalled();
});

test("keeps one Monitor subscription while demand revisions change", async () => {
  const onMediaChanged = vi.fn(async () => () => undefined);
  const onCacheProcessorWarning = vi.fn(async () => () => undefined);
  const prepareMediaPreviews = vi.fn(async () => []);

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged,
        onCacheProcessorWarning,
      }}
      projectCorePort={projectCorePort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  fireEvent.click(
    screen.getByRole("button", { name: "Esvaziar demanda de Canvas" }),
  );
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));

  expect(onMediaChanged).toHaveBeenCalledOnce();
  expect(onCacheProcessorWarning).toHaveBeenCalledOnce();
  expect(prepareMediaPreviews).toHaveBeenNthCalledWith(2, {
    revision: 2,
    visibleMediaIds: [],
    preloadMediaIds: [],
  }, expect.any(Function));
});

test("cancels resident media demand when runtime graphics become unavailable", async () => {
  const prepareMediaPreviews = vi.fn(async () => []);
  const dialog = projectDialogHarness();
  let resolveGraphicsDialogPresentation: (() => void) | undefined;
  dialog.present.mockImplementationOnce(
    () =>
      new Promise<undefined>((resolve) => {
        resolveGraphicsDialogPresentation = () => resolve(undefined);
      }),
  );
  const requestClose = vi.fn(async () => ({ kind: "closed" as const }));

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={dialog.port}
      projectWindowPort={{ ...projectWindowPort, requestClose }}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={projectCorePort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  fireEvent.click(
    screen.getByRole("button", { name: "Simular perda grafica" }),
  );
  await waitFor(() =>
    expect(dialog.present).toHaveBeenCalledWith({
      kind: "graphicsFailure",
      reason: "WebGL2 runtime failure.",
    }),
  );
  expect(document.querySelector(".workspace-grid")).toHaveAttribute("inert");
  expect(document.querySelector(".workspace-grid")).toHaveAttribute(
    "aria-busy",
    "true",
  );
  expect(
    screen.getByRole("button", { name: "Exportar Lâmina" }),
  ).toBeDisabled();
  expect(screen.getByTestId("album-canvas")).toBeInTheDocument();
  expect(
    screen.queryByRole("heading", { name: /O Canvas/ }),
  ).not.toBeInTheDocument();
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
  expect(prepareMediaPreviews).toHaveBeenNthCalledWith(2, {
    revision: 2,
    visibleMediaIds: [],
    preloadMediaIds: [],
  }, expect.any(Function));

  act(() => resolveGraphicsDialogPresentation?.());
  act(() => dialog.emit("closeProjectAfterGraphicsFailure"));
  await waitFor(() => expect(dialog.dismiss).toHaveBeenCalledOnce());
  expect(requestClose).toHaveBeenCalledOnce();
});

test("routes a dirty defensive graphics failure through the existing close confirmation", async () => {
  const sessions: Array<{
    dismiss: ReturnType<typeof vi.fn>;
    listener: (action: ProjectDialogAction) => void;
    present: ReturnType<typeof vi.fn>;
  }> = [];
  const dialogPort: ProjectDialogPort = {
    acquire: (listener) => {
      const session = {
        dismiss: vi.fn(async () => undefined),
        listener,
        present: vi.fn(async () => undefined),
      };
      sessions.push(session);
      return session;
    },
  };
  const requestClose = vi.fn(async () => ({
    kind: "confirmationRequired" as const,
  }));
  const resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection,
  }));

  render(
    <App
      workspacePreferencesMode="memory"
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={dialogPort}
      projectWindowPort={{
        ...projectWindowPort,
        requestClose,
        resolveClose,
      }}
      mediaPreviewPort={mediaPreviewPort}
      projectCorePort={projectCorePort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: false,
        code: "webgl2_unavailable",
        renderer: "indisponível",
        reason: "WebGL2 acelerado por hardware não foi confirmado.",
        limits: null,
      })}
    />,
  );

  await waitFor(() => expect(sessions).toHaveLength(1));
  expect(sessions[0]?.present).toHaveBeenCalledWith({
    kind: "graphicsFailure",
    reason: "WebGL2 acelerado por hardware não foi confirmado.",
  });

  act(() =>
    sessions[0]?.listener("closeProjectAfterGraphicsFailure"),
  );
  await waitFor(() => expect(requestClose).toHaveBeenCalledOnce());
  await waitFor(() => expect(sessions).toHaveLength(2));
  expect(sessions[1]?.present).toHaveBeenCalledWith({
    busy: false,
    kind: "projectCloseConfirmation",
  });

  act(() => sessions[1]?.listener("cancelProjectClose"));
  await waitFor(() => expect(resolveClose).toHaveBeenCalledWith("cancel"));
  await waitFor(() => expect(sessions[1]?.dismiss).toHaveBeenCalledOnce());
  await waitFor(() => expect(sessions).toHaveLength(3));
  expect(sessions[2]?.present).toHaveBeenCalledWith({
    kind: "graphicsFailure",
    reason: "WebGL2 acelerado por hardware não foi confirmado.",
  });
});

test("does not treat a rejected close confirmation as an explicit graphics-close cancellation", async () => {
  const sessions: Array<{
    dismiss: ReturnType<typeof vi.fn>;
    listener: (action: ProjectDialogAction) => void;
    present: ReturnType<typeof vi.fn>;
  }> = [];
  const dialogPort: ProjectDialogPort = {
    acquire: (listener) => {
      const session = {
        dismiss: vi.fn(async () => undefined),
        listener,
        present: vi.fn(async () => {
          if (sessions.length <= 2) {
            throw new Error("A janela pertencente não pôde ser apresentada.");
          }
        }),
      };
      sessions.push(session);
      return session;
    },
  };
  const requestClose = vi.fn(async () => ({
    kind: "confirmationRequired" as const,
  }));
  const resolveClose = vi.fn(async () => ({
    kind: "cancelled" as const,
    projection,
  }));

  render(
    <App
      workspacePreferencesMode="memory"
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={dialogPort}
      projectWindowPort={{
        ...projectWindowPort,
        requestClose,
        resolveClose,
      }}
      mediaPreviewPort={mediaPreviewPort}
      projectCorePort={projectCorePort}
      logger={silentLogger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: false,
        code: "webgl2_unavailable",
        renderer: "indisponível",
        reason: "WebGL2 acelerado por hardware não foi confirmado.",
        limits: null,
      })}
    />,
  );

  await waitFor(() => expect(resolveClose).toHaveBeenCalledWith("cancel"));
  await new Promise((resolve) => setTimeout(resolve, 50));
  expect(sessions[0]?.present).toHaveBeenCalledWith({
    kind: "graphicsFailure",
    reason: "WebGL2 acelerado por hardware não foi confirmado.",
  });
  expect(sessions[1]?.present).toHaveBeenCalledWith({
    busy: false,
    kind: "projectCloseConfirmation",
  });
  expect(sessions[2]?.present).toHaveBeenCalledWith({
    kind: "projectOperationFailure",
    message: "A janela pertencente não pôde ser apresentada.",
  });
  expect(
    sessions.flatMap((session) => session.present.mock.calls).filter(
      ([state]) => state.kind === "graphicsFailure",
    ),
  ).toHaveLength(1);
});

test("logs the typed media preview failure code without replacing it with unknown_error", async () => {
  const logEvents: LogEvent[] = [];
  const logger: Logger = {
    write: (event) => logEvents.push(event),
  };
  const failure = Object.assign(
    new Error("A Imagem decorativa vinculada não está disponível."),
    { code: "unavailable" },
  );
  const prepareMediaPreviews = vi.fn(async () => {
    throw failure;
  });

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
        onCacheProcessorWarning: async () => () => undefined,
      }}
      projectCorePort={projectCorePort}
      logger={logger}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
        limits: {
          maxTextureSizePx: 16_384,
          maxRenderbufferSizePx: 16_384,
          maxTextureImageUnits: 16,
        },
      })}
    />,
  );

  await waitFor(() =>
    expect(logEvents).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          component: "media-preview",
          event: "media_preview_failed",
          reason: "unavailable",
        }),
      ]),
    ),
  );
});

test("keeps a created Project usable while reporting initial image cache problems", async () => {
  const dialog = projectDialogHarness();
  const problem = { fileName: "Fundo.png", reason: "A imagem foi vinculada, mas sua prévia não pôde ser preparada." };
  let warn: Parameters<MediaPreviewPort["onCacheProcessorWarning"]>[0] | undefined;
  const confirmUiReady = vi.fn(async () => {
    warn?.({ state: "suspended", message: "O Cache foi suspenso." });
    return [problem];
  });
  render(<App
    exportPipelinePort={exportPipelinePort}
    mediaPreviewPort={{ ...mediaPreviewPort, onCacheProcessorWarning: async (listener) => {
      warn = listener;
      return () => undefined;
    } }}
    projectStartupPort={{ confirmUiReady }}
    projectCorePort={projectCorePort}
    projectWindowPort={projectWindowPort}
    projectDialogPort={dialog.port}
    graphicsProbe={canvasGraphicsDiagnosticProbe}
    canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
    logger={silentLogger}
  />);
  await waitFor(() => expect(dialog.present).toHaveBeenCalledWith({
    kind: "projectOperationFailure", message: "O Cache foi suspenso.",
  }));
  act(() => dialog.emit("dismissProjectOperationFailure"));
  await waitFor(() => expect(dialog.present).toHaveBeenCalledWith({
    kind: "imageProcessingProblems", importedCount: null, problems: [problem],
  }));
  expect(confirmUiReady).toHaveBeenCalledOnce();
  expect(screen.getByTestId("album-canvas")).toBeInTheDocument();
  act(() => dialog.emit("dismissImageProcessingProblems"));
  await waitFor(() => expect(dialog.dismiss).toHaveBeenCalled());
});

test.each([
  { outcome: "success", alreadyReading: false },
  { outcome: "failure", alreadyReading: false },
  { outcome: "success", alreadyReading: true },
])("reveals an import batch after $outcome when the monitor is already reading: $alreadyReading", async ({ outcome, alreadyReading }) => {
  let notify: Parameters<MediaPreviewPort["onMediaChanged"]>[0] = () => undefined;
  let progress: Parameters<ProjectCorePort["importPhoto"]>[0] = () => undefined;
  let finish!: (result: Awaited<ReturnType<ProjectCorePort["importPhoto"]>>) => void;
  let fail!: (reason: Error) => void;
  let finishEarlyRead!: (projection: typeof representativeProjection) => void;
  const newPhotos = [1, 2].map((number) => ({
    ...representativeProjection.state.album.media.find((media) => media.kind === "photo")!,
    id: `new-photo-${number}`,
    name: `Nova ${number}.jpg`,
  }));
  const imported = {
    ...representativeProjection,
    state: {
      ...representativeProjection.state,
      revision: representativeProjection.state.revision + 1,
      album: {
        ...representativeProjection.state.album,
        media: [...representativeProjection.state.album.media, ...newPhotos],
      },
    },
  };
  const load = vi.fn().mockResolvedValueOnce(representativeProjection);
  if (alreadyReading) {
    load.mockImplementationOnce(() => new Promise((resolve) => {
      finishEarlyRead = resolve;
    }));
  }
  load.mockResolvedValue(imported);
  const importPhoto = vi.fn<ProjectCorePort["importPhoto"]>((onProgress) => {
    progress = onProgress;
    return new Promise((resolve, reject) => {
      finish = resolve;
      fail = reject;
    });
  });
  render(
    <App
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      projectCorePort={{ ...projectCorePort, load, importPhoto }}
      mediaPreviewPort={{
        ...mediaPreviewPort,
        onMediaChanged: async (listener) => {
          notify = listener;
          return () => undefined;
        },
      }}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      logger={silentLogger}
    />,
  );
  await screen.findByRole("button", { name: /Serra ao amanhecer\.jpg/i });
  if (alreadyReading) {
    act(() => notify(["media-001"]));
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  }
  fireEvent.click(screen.getByRole("button", { name: "Importar" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos JPEG…" }));
  await waitFor(() => expect(importPhoto).toHaveBeenCalledOnce());
  if (alreadyReading) {
    await act(async () => finishEarlyRead(imported));
    expect(screen.queryByRole("button", { name: "Nova 1.jpg" })).not.toBeInTheDocument();
  }
  await act(async () => {
    progress?.({ completedFiles: 0, totalFiles: 2 });
    notify(newPhotos.map(({ id }) => id));
  });
  await act(async () => {
    progress?.({ completedFiles: 1, totalFiles: 2 });
    notify([newPhotos[0].id]);
    notify([newPhotos[1].id]);
  });
  expect(load).toHaveBeenCalledTimes(alreadyReading ? 2 : 1);
  expect(screen.getByRole("button", { name: /Serra ao amanhecer\.jpg/i })).toBeVisible();
  expect(screen.queryByRole("button", { name: "Nova 1.jpg" })).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: "Nova 2.jpg" })).not.toBeInTheDocument();
  await act(async () => {
    if (outcome === "failure") {
      fail(new Error("O processamento falhou após vincular as imagens."));
    } else {
      finish({
        kind: "completed",
        projection: imported,
        mediaIds: newPhotos.map(({ id }) => id),
        importedCount: 2,
        problems: [],
      });
    }
  });
  expect(await screen.findByRole("button", { name: "Nova 1.jpg" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Nova 2.jpg" })).toBeVisible();
  await waitFor(() => expect(load).toHaveBeenCalledTimes(alreadyReading ? 3 : 2));
});

test("keeps a completed Save authoritative when a monitor read finishes during saving", async () => {
  let notify: Parameters<MediaPreviewPort["onMediaChanged"]>[0] = () => undefined;
  let finishRead!: (projection: typeof representativeProjection) => void;
  let finishSave!: (result: Awaited<ReturnType<ProjectCorePort["save"]>>) => void;
  const dirty = {
    ...representativeProjection,
    state: { ...representativeProjection.state, dirty: true },
  };
  const saved = {
    ...dirty,
    state: { ...dirty.state, dirty: false, savedRevision: dirty.state.revision },
  };
  const load = vi.fn()
    .mockResolvedValueOnce(dirty)
    .mockImplementationOnce(() => new Promise((resolve) => { finishRead = resolve; }))
    .mockResolvedValue(saved);
  const save = vi.fn<ProjectCorePort["save"]>(() => new Promise((resolve) => {
    finishSave = resolve;
  }));
  render(
    <App
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
      projectCorePort={{ ...projectCorePort, load, save }}
      mediaPreviewPort={{ ...mediaPreviewPort, onMediaChanged: async (listener) => {
        notify = listener;
        return () => undefined;
      } }}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      logger={silentLogger}
    />,
  );
  await screen.findByText("alterações não salvas");
  act(() => notify(["media-001"]));
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  fireEvent.click(screen.getByRole("menuitem", { name: "Arquivo" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Salvar" }));
  await waitFor(() => expect(save).toHaveBeenCalledOnce());
  await act(async () => finishRead(dirty));
  await act(async () => finishSave({
    outcome: { kind: "saved", revision: saved.state.revision },
    projection: saved,
  }));
  await waitFor(() => expect(screen.queryByText("alterações não salvas")).not.toBeInTheDocument());
});

test.each(["ready", "decode_failed", "native_unavailable"] as const)("delivers imported cards together after visible preview preparation: %s", async (outcome) => {
  const dialog = projectDialogHarness();
  const photos = [1, 2].map((number) => ({
    ...representativeProjection.state.album.media[0],
    kind: "photo" as const, id: `batch-${number}`, name: `Batch ${number}.jpg`,
  }));
  const imported = {
    ...representativeProjection,
    state: { ...representativeProjection.state,
      revision: representativeProjection.state.revision + 1,
      album: { ...representativeProjection.state.album,
        media: [...representativeProjection.state.album.media, ...photos] },
    },
  };
  let finishPreviews!: (previews: readonly MediaPreview[]) => void;
  let finishImport!: () => void;
  const close = vi.fn(projectWindowPort.requestClose);
  let publishPreview: ((preview: MediaPreview) => void) | undefined;
  const decodeReady = new Map<string, () => void>();
  vi.stubGlobal("Image", class {
    src = "";
    decode() {
      if (!this.src.includes("batch-")) return Promise.resolve();
      return new Promise<void>((resolve, reject) => decodeReady.set(this.src,
        outcome === "decode_failed" && this.src.includes("batch-2")
          ? () => reject(new Error("Image decode failed")) : resolve,
      ));
    }
  });
  const prepare = vi.fn<MediaPreviewPort["prepareMediaPreviews"]>(async (demand, publish) => {
    if (!demand.visibleMediaIds.includes("batch-1")) return [];
    publishPreview = publish;
    return new Promise((resolve) => { finishPreviews = resolve; });
  });
  try {
    render(<App
      projectStartupPort={projectStartupPort} projectWindowPort={{ ...projectWindowPort,
        requestClose: close,
      }}
      projectDialogPort={dialog.port}
      projectCorePort={{ ...projectCorePort,
        load: async () => representativeProjection,
        importPhoto: async (publish) => {
          publish?.({ completedFiles: 2, totalFiles: 2 });
          await new Promise<void>((resolve) => { finishImport = resolve; });
          return { kind: "completed", projection: imported,
            mediaIds: photos.map(({ id }) => id), importedCount: 2, problems: [] };
        },
      }}
      mediaPreviewPort={{ ...mediaPreviewPort, prepareMediaPreviews: prepare }}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe} logger={silentLogger}
    />);
    const grid = await screen.findByRole("group", { name: "Grade de Fotos" });
    Object.defineProperties(grid, {
      clientWidth: { value: 600 }, clientHeight: { value: 200 },
    });
    fireEvent.click(screen.getByRole("button", { name: "Importar" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Arquivos JPEG…" }));
    await waitFor(() => expect(dialog.present).toHaveBeenCalledWith({
      kind: "imageProcessingProgress",
      progress: { kind: "determinate", completed: 2, total: 2, status: "2 de 2" },
    }));
    await act(async () => finishImport());
    expect(screen.queryByRole("button", { name: "Batch 1.jpg" })).not.toBeInTheDocument();
    await waitFor(() => expect(finishPreviews).toBeTypeOf("function"));
    const previews: MediaPreview[] = photos.map(({ id }) => ({
      mediaId: id, state: "ready", url: `https://preview.test/${id}.jpg`,
    }));
    if (outcome === "native_unavailable") previews[1] = { mediaId: "batch-2", state: "unavailable", url: null };
    await act(async () => publishPreview?.(previews[0]));
    expect(screen.queryByRole("button", { name: "Batch 1.jpg" })).not.toBeInTheDocument();
    await act(async () => finishPreviews(previews));
    await waitFor(() => expect(decodeReady.size).toBe(outcome === "native_unavailable" ? 1 : 2));
    fireEvent.click(screen.getByRole("menuitem", { name: "Arquivo" }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Fechar Projeto" }));
    expect(close).not.toHaveBeenCalled();
    expect(screen.queryByRole("button", { name: "Batch 1.jpg" })).not.toBeInTheDocument();
    expect(dialog.dismiss).not.toHaveBeenCalled();
    await act(async () => decodeReady.get(previews[0].url!)!());
    if (outcome !== "native_unavailable") {
      expect(screen.queryByRole("button", { name: "Batch 1.jpg" })).not.toBeInTheDocument();
      expect(dialog.dismiss).not.toHaveBeenCalled();
      await act(async () => decodeReady.get(previews[1].url!)!());
    }
    for (const photo of photos) {
      if (outcome !== "ready" && photo.id === "batch-2") {
        const status = outcome === "native_unavailable" ? "Indisponível" : "Prévia indisponível";
        expect((await screen.findByRole("button", { name: `${photo.name}. ${status}` }))
          .querySelector("img")).toBeNull();
      } else {
        expect(await screen.findByRole("button", { name: photo.name }))
          .toContainHTML(`src="https://preview.test/${photo.id}.jpg"`);
      }
    }
    await waitFor(() => expect(dialog.dismiss).toHaveBeenCalled());
    await waitFor(() => expect(close).toHaveBeenCalledOnce());
    expect(prepare.mock.calls.filter(([demand]) => demand.visibleMediaIds.includes("batch-1")))
      .toHaveLength(1);
    if (outcome !== "ready") {
      expect(dialog.present).toHaveBeenCalledWith(expect.objectContaining({
        kind: "imageProcessingProblems", problems: [expect.objectContaining({ fileName: "Batch 2.jpg" })],
      }));
    }
  } finally {
    vi.unstubAllGlobals();
  }
});
