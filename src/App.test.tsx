import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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
  ProjectSessionPort,
} from "./application/projectPorts";
import type { TopologyFaultProbeBridge } from "./application/topologyFaultProbe";
import type { EditorProjection } from "./domain/project";
import {
  createEmptyProjection,
  representativeProjection,
} from "./test/projectFixtures";

vi.mock("./components/AlbumCanvas", () => ({
  AlbumCanvas: () => <div data-testid="album-canvas" />,
}));

const projection = createEmptyProjection();

const projectSessionPort: ProjectSessionPort = {
  load: async () => projection,
  apply: async () => projection,
  undo: async () => projection,
  redo: async () => projection,
};
const mediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: async () => null,
};
const exportPort: ExportPort = {
  startPreview: () => ({
    completion: Promise.resolve({
      status: "completed",
      result: {
        outputPath: "C:\\Temp\\Album-Horizonte_001.png",
        widthPx: 600,
        heightPx: 300,
      },
    }),
    cancel: async () => "not_found",
  }),
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

test("keeps diagnostics available when hardware WebGL2 is unavailable", async () => {
  const user = userEvent.setup();
  const load = vi.fn(async () => projection);
  const prepareMediaPreviews = vi.fn(async () => null);
  render(
    <App
      exportPort={exportPort}
      mediaPreviewPort={{ prepareMediaPreviews }}
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
    await screen.findByRole("heading", { name: "Boas-vindas" }),
  ).toBeInTheDocument();
  expect(
    screen.getByText("WebGL2 acelerado por hardware não foi confirmado."),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("navigation", { name: "Superfícies globais" }),
  ).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Configurações" }));
  expect(
    screen.getByRole("heading", {
      name: "Configurações do aplicativo",
    }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("tab", { name: "Desempenho" }),
  ).toHaveAttribute("aria-selected", "true");
  await user.click(screen.getByRole("tab", { name: "Photoshop" }));
  expect(
    screen.getByRole("heading", { name: "Photoshop" }),
  ).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Diagnóstico" }));
  expect(
    screen.getByRole("heading", { name: "Diagnóstico gráfico" }),
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
  render(
    <App
      exportPort={exportPort}
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
    await screen.findByRole("button", { name: "Exportar prova" }),
  ).toBeInTheDocument();
  expect(
    screen.getByRole("navigation", { name: "Menu principal" }),
  ).toBeInTheDocument();
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
        url: "asset://localhost/cache/media-001.jpg",
      },
    ]);

  render(
    <App
      exportPort={exportPort}
      mediaPreviewPort={{ prepareMediaPreviews }}
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
    await screen.findByRole("button", { name: "Exportar prova" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(prepareMediaPreviews).toHaveBeenCalledOnce());
  expect(logEvents).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        component: "media-cache",
        event: "media_cache_completed",
        projectId: projection.state.projectId,
      }),
    ]),
  );
});

test("serializes fault-probe and editor changes through one Project queue", async () => {
  const canonical = withLeadingPlaceholder(representativeProjection);
  const appliedProbe = withRevision(canonical, 26, true);
  const savedProbe = {
    ...withRevision(appliedProbe, 26, false),
    state: {
      ...appliedProbe.state,
      savedRevision: 26,
      dirty: false,
    },
  };
  const appliedEditor = withRevision(savedProbe, 27, true);
  let releaseCanonical!: (projection: EditorProjection) => void;
  const canonicalPending = new Promise<EditorProjection>((resolve) => {
    releaseCanonical = resolve;
  });
  const load = vi
    .fn<ProjectSessionPort["load"]>()
    .mockResolvedValueOnce(canonical)
    .mockImplementationOnce(() => canonicalPending);
  const apply = vi
    .fn<ProjectSessionPort["apply"]>()
    .mockResolvedValueOnce(appliedProbe)
    .mockResolvedValueOnce(appliedEditor);
  const topologyFaultProbeBridge: TopologyFaultProbeBridge = {
    enabled: true,
    loadConfig: vi.fn(async () => ({
      enabled: true,
      config: {
        probeId: "shared-project-queue",
        expectedGlobalAvailable: true,
      },
    })),
    persistAndReport: vi.fn(async () => ({
      projection: savedProbe,
      probeId: "shared-project-queue",
      previousRevision: 25,
      persistedRevision: 26,
      bytes: 4_096,
      sha256: "7f83b1657ff1fc53b92dc18148a1d65dfa13514d",
      globalAvailable: true,
      globalProcessId: 1_234,
      globalRoundTripMs: 1.5,
    })),
    reportFailure: vi.fn(),
  };

  render(
    <App
      exportPort={exportPort}
      mediaPreviewPort={mediaPreviewPort}
      projectSessionPort={{
        ...projectSessionPort,
        load,
        apply,
      }}
      topologyFaultProbeBridge={topologyFaultProbeBridge}
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
    await screen.findByRole("button", { name: "Exportar prova" }),
  ).toBeInTheDocument();
  await waitFor(() => expect(load).toHaveBeenCalledTimes(2));

  fireEvent.doubleClick(
    screen.getByText("Serra ao amanhecer.jpg").closest("button")!,
  );
  expect(apply).not.toHaveBeenCalled();

  await act(async () => {
    releaseCanonical(canonical);
    await canonicalPending;
  });

  await waitFor(() => expect(apply).toHaveBeenCalledTimes(2));
  expect(apply).toHaveBeenNthCalledWith(
    1,
    expect.objectContaining({ kind: "transformPhoto" }),
  );
  expect(apply).toHaveBeenNthCalledWith(2, {
    kind: "fillLeftmostPlaceholder",
    sheetId: "sheet-001",
    mediaId: "media-001",
  });
});

function withLeadingPlaceholder(
  source: EditorProjection,
): EditorProjection {
  const firstSheet = source.state.album.sheets[0];
  return {
    ...source,
    state: {
      ...source.state,
      album: {
        ...source.state.album,
        sheets: [
          {
            ...firstSheet,
            frames: [
              {
                ...firstSheet.frames[0],
                id: "placeholder-001",
                photo: null,
              },
              ...firstSheet.frames,
            ],
          },
        ],
      },
    },
  };
}

function withRevision(
  source: EditorProjection,
  revision: number,
  dirty: boolean,
): EditorProjection {
  return {
    ...source,
    state: {
      ...source.state,
      revision,
      dirty,
    },
  };
}
