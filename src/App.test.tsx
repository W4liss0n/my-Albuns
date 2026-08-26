import { useEffect } from "react";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import App from "./App";
import {
  type LogEvent,
  type Logger,
  silentLogger,
} from "./application/logging";
import type {
  ExportPort,
  MediaPreviewPort,
  ProjectStartupPort,
  ProjectSessionPort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { ProjectDialogPort } from "./application/projectDialogPort";
import {
  createWorkspacePreferences,
  type WorkspacePreferences,
} from "./application/workspacePreferences";
import {
  createEmptyProjection,
  representativeProjection,
} from "./test/projectFixtures";

vi.mock("./components/AlbumCanvas", () => ({
  AlbumCanvas: ({
    onMediaDemandChange,
    mediaPreviewUrls,
    onGraphicsUnavailable,
  }: {
    onMediaDemandChange?: (demand: {
      visibleMediaIds: readonly string[];
      preloadMediaIds: readonly string[];
    }) => void;
    mediaPreviewUrls?: Readonly<Record<string, string>>;
    onGraphicsUnavailable?: (diagnostic: {
      supported: false;
      code: "webgl2_unavailable";
      renderer: string;
      reason: string;
      limits: null;
    }) => void;
  }) => {
    useEffect(() => {
      onMediaDemandChange?.({
        visibleMediaIds: ["media-001"],
        preloadMediaIds: [],
      });
    }, [onMediaDemandChange]);
    return (
      <>
        <div
          data-testid="album-canvas"
          data-media-preview={mediaPreviewUrls?.["media-001"] ?? ""}
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

const projectSessionPort: ProjectSessionPort = {
  load: async () => projection,
  validateAlbumInformation: async () => ({
    errors: [],
    impact: { sheetWidthPx: 7_087, pageWidthPx: 3_543, heightPx: 3_543 },
  }),
  apply: async () => projection,
  undo: async () => projection,
  redo: async () => projection,
  save: async () => {
    throw new Error("Salvamento não configurado neste teste.");
  },
};
const mediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: async () => null,
  onMediaChanged: async () => () => undefined,
};
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

test("reports a defensive Project Canvas failure without claiming that no Session exists", async () => {
  const load = vi.fn(async () => projection);
  const prepareMediaPreviews = vi.fn(async () => null);
  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
      }}
      projectSessionPort={{ ...projectSessionPort, load }}
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
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      projectStartupPort={{ confirmUiReady }}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={mediaPreviewPort}
      projectSessionPort={{ ...projectSessionPort, load }}
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
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
      }}
      projectSessionPort={projectSessionPort}
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
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        prepareMediaPreviews,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
      }}
      projectSessionPort={{
        ...projectSessionPort,
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
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        prepareMediaPreviews,
        onMediaChanged: async (listener) => {
          notifyMediaChanged = listener;
          return () => undefined;
        },
      }}
      projectSessionPort={{
        ...projectSessionPort,
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

test("labels first-observation unavailability without claiming a previous preview", async () => {
  const prepareMediaPreviews = vi.fn().mockResolvedValue([
    { mediaId: "media-001", state: "unavailable" as const, url: null },
  ]);

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
      }}
      projectSessionPort={{
        ...projectSessionPort,
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
  expect(screen.getByRole("status", { name: "Indisponível" })).toBeInTheDocument();
  expect(screen.queryByText("Indisponível · prévia anterior")).not.toBeInTheDocument();
  expect(screen.getByTestId("album-canvas")).toHaveAttribute(
    "data-media-preview",
    "",
  );
});

test("keeps one Monitor subscription while demand revisions change", async () => {
  const onMediaChanged = vi.fn(async () => () => undefined);
  const prepareMediaPreviews = vi.fn(async () => []);

  render(
    <App
      workspacePreferencesMode="memory"
      exportPort={exportPort}
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{ prepareMediaPreviews, onMediaChanged }}
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
    />,
  );

  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  fireEvent.click(
    screen.getByRole("button", { name: "Esvaziar demanda de Canvas" }),
  );
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledTimes(2));

  expect(onMediaChanged).toHaveBeenCalledOnce();
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
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
      }}
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
      projectStartupPort={projectStartupPort}
      projectDialogPort={projectDialogPort}
      projectWindowPort={projectWindowPort}
      mediaPreviewPort={{
        prepareMediaPreviews,
        onMediaChanged: async () => () => undefined,
      }}
      projectSessionPort={projectSessionPort}
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
