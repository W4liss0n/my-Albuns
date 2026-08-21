import { useEffect, useState } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import App from "./App";
import {
  type LogEvent,
  type Logger,
  silentLogger,
} from "./application/logging";
import type {
  ExportPipelinePort,
  MediaPreview,
  MediaPreviewPort,
  ProjectStartupPort,
  ProjectCorePort,
  ProjectWindowPort,
} from "./application/projectPorts";
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
  apply: async () => projection,
  applyWithOutcome: async () => ({
    projection,
    affectedFrameId: null,
  }),
  importPhoto: async () => ({ kind: "cancelled", projection }),
  resolvePhotoDropTarget: async () => ({ kind: "invalid" }),
  relink: async () => projection,
  undo: async () => projection,
  redo: async () => projection,
  save: async () => {
    throw new Error("Salvamento não configurado neste teste.");
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
const projectWindowPort: ProjectWindowPort = {
  onCloseRequested: async () => () => undefined,
  requestClose: async () => ({ kind: "closed" }),
  resolveClose: async () => ({ kind: "closed" }),
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

test("reports a defensive Project Canvas failure without claiming that no Session exists", async () => {
  const load = vi.fn(async () => projection);
  const prepareMediaPreviews = vi.fn(async () => null);
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
  const confirmUiReady = vi.fn(async () => undefined);
  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={{ confirmUiReady }}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={mediaPreviewPort}
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
    screen.getByRole("navigation", { name: "Menu principal" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(confirmUiReady).toHaveBeenCalledOnce());
  expect(screen.queryByText("Álbum Horizonte")).not.toBeInTheDocument();
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
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
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
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
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
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
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

test("shows a non-blocking warning when repeated processor failures suspend Cache", async () => {
  let warnCacheSuspended:
    | ((warning: { state: "suspended"; message: string }) => void)
    | undefined;
  render(
    <App
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
      projectWindowPort={projectWindowPort}
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

  await screen.findByRole("button", { name: "Salvar" });
  act(() =>
    warnCacheSuspended?.({
      state: "suspended",
      message:
        "O Cache foi suspenso após falhas repetidas do Processador de Imagens.",
    }),
  );

  expect(
    screen.getByRole("status", { name: "Cache suspenso" }),
  ).toHaveTextContent(
    "O Cache foi suspenso após falhas repetidas do Processador de Imagens.",
  );
  expect(screen.getByRole("button", { name: "Salvar" })).toBeEnabled();
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

  await screen.findByRole("button", { name: "Salvar" });
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
  expect(
    screen.getByRole("status", { name: "Cache suspenso" }),
  ).toHaveTextContent(
    "O Cache foi suspenso após falhas repetidas do Processador de Imagens.",
  );
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

  await screen.findByRole("button", { name: "Salvar" });
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

  await screen.findByText("Serra ao amanhecer.jpg");
  act(() => {
    notifyMediaChanged?.(["media-001"]);
    notifyMediaChanged?.(["media-001"]);
  });
  await waitFor(() => expect(load).toHaveBeenCalledTimes(3));
  await act(async () => {
    resolveNewer(projectionNamed("Observação nova.jpg"));
  });
  await screen.findByText("Observação nova.jpg");

  await act(async () => {
    resolveOlder(projectionNamed("Observação antiga.jpg"));
  });
  expect(screen.getByText("Observação nova.jpg")).toBeInTheDocument();
  expect(screen.queryByText("Observação antiga.jpg")).not.toBeInTheDocument();
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
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
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
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
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
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
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
      exportPipelinePort={exportPipelinePort}
      projectStartupPort={projectStartupPort}
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
