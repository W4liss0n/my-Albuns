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
  ProjectStartupPort,
  ProjectCorePort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { ProjectDialogPort } from "./application/projectDialogPort";
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
  recoveryStatus: async () => ({ kind: "none" }),
  resolveRecovery: async () => {
    throw new Error("Recuperação não configurada neste teste.");
  },
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
  return {
    dismiss,
    present,
    port: {
      acquire: () => ({ dismiss, present }),
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

test("offers exactly the three Recovery choices before reading editor state", async () => {
  const load = vi.fn(async () => projection);
  const recoveryStatus = vi.fn(async () => ({ kind: "available" }) as const);
  const recoveredProjection: EditorProjection = {
    ...projection,
    state: {
      ...projection.state,
      dirty: true,
      canUndo: false,
      canRedo: false,
      document: { ...projection.state.document, dpi: 360 },
    },
  };
  const resolveRecovery = vi.fn(async () => ({
    kind: "recovered" as const,
    projection: recoveredProjection,
  }));
  const confirmUiReady = vi.fn(async () => undefined);

  render(
    <App
      exportPipelinePort={exportPipelinePort}
      mediaPreviewPort={mediaPreviewPort}
      projectStartupPort={{
        recoveryStatus,
        resolveRecovery,
        confirmUiReady,
      }}
      projectCorePort={{ ...projectCorePort, load }}
      projectWindowPort={projectWindowPort}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      logger={silentLogger}
    />,
  );

  expect(
    await screen.findByRole("heading", { name: "Recuperar trabalho não salvo?" }),
  ).toBeInTheDocument();
  expect(screen.getAllByRole("button").map((button) => button.textContent)).toEqual([
    "Reabrir e recuperar",
    "Abrir última versão salva",
    "Agora não",
  ]);
  expect(load).not.toHaveBeenCalled();
  await waitFor(() => expect(confirmUiReady).toHaveBeenCalledOnce());

  fireEvent.click(
    screen.getByRole("button", { name: "Reabrir e recuperar" }),
  );
  await waitFor(() =>
    expect(resolveRecovery).toHaveBeenCalledWith("reopenAndRecover"),
  );
  expect(
    await screen.findByRole("button", { name: "Exportar Lâmina" }),
  ).toBeInTheDocument();
  expect(load).not.toHaveBeenCalled();
});

test("requires a separate confirmation before discarding Recovery for the saved version", async () => {
  const load = vi.fn(async () => projection);
  const resolveRecovery = vi.fn(async () => ({
    kind: "openedLastSaved" as const,
    projection,
  }));
  render(
    <App
      exportPipelinePort={exportPipelinePort}
      mediaPreviewPort={mediaPreviewPort}
      projectStartupPort={{
        ...projectStartupPort,
        recoveryStatus: async () => ({ kind: "available" }),
        resolveRecovery,
      }}
      projectCorePort={{ ...projectCorePort, load }}
      projectWindowPort={projectWindowPort}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      logger={silentLogger}
    />,
  );

  fireEvent.click(
    await screen.findByRole("button", { name: "Abrir última versão salva" }),
  );
  expect(resolveRecovery).not.toHaveBeenCalled();
  expect(
    screen.getByRole("heading", { name: "Descartar o trabalho recuperável?" }),
  ).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByRole("button", { name: "Abrir última versão salva" }),
  ).toBeInTheDocument();
  expect(resolveRecovery).not.toHaveBeenCalled();

  fireEvent.click(
    screen.getByRole("button", { name: "Abrir última versão salva" }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Descartar recuperação e abrir" }),
  );
  await waitFor(() =>
    expect(resolveRecovery).toHaveBeenCalledWith(
      "discardCheckpointAndOpenLastSaved",
    ),
  );
  expect(load).not.toHaveBeenCalled();
});

test("defers opening without loading or discarding the Recovery checkpoint", async () => {
  const load = vi.fn(async () => projection);
  const resolveRecovery = vi.fn(async () => ({ kind: "deferred" as const }));
  render(
    <App
      exportPipelinePort={exportPipelinePort}
      mediaPreviewPort={mediaPreviewPort}
      projectStartupPort={{
        ...projectStartupPort,
        recoveryStatus: async () => ({ kind: "available" }),
        resolveRecovery,
      }}
      projectCorePort={{ ...projectCorePort, load }}
      projectWindowPort={projectWindowPort}
      graphicsProbe={canvasGraphicsDiagnosticProbe}
      canvasGraphicsDiagnosticProbe={canvasGraphicsDiagnosticProbe}
      logger={silentLogger}
    />,
  );

  fireEvent.click(await screen.findByRole("button", { name: "Agora não" }));
  await waitFor(() =>
    expect(resolveRecovery).toHaveBeenCalledWith("nowNot"),
  );
  expect(load).not.toHaveBeenCalled();
  expect(screen.getByText("Fechando o Projeto…")).toBeInTheDocument();
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

  expect(
    await screen.findByRole("heading", {
      name: "O Canvas não pôde ser iniciado",
    }),
  ).toBeInTheDocument();
  expect(
    screen.getByText("WebGL2 acelerado por hardware não foi confirmado."),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("alert"),
  ).toBeInTheDocument();

  expect(load).not.toHaveBeenCalled();
  expect(prepareMediaPreviews).not.toHaveBeenCalled();
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
    expect(retryUnavailableMedia).toHaveBeenCalledWith("media-001"),
  );
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
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
  });
});

test("cancels resident media demand when runtime graphics become unavailable", async () => {
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
  expect(
    await screen.findByRole("heading", {
      name: /O Canvas/,
    }),
  ).toBeInTheDocument();
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));
  expect(prepareMediaPreviews).toHaveBeenNthCalledWith(2, {
    revision: 2,
    visibleMediaIds: [],
    preloadMediaIds: [],
  });
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
