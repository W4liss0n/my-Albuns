import { render, screen, waitFor } from "@testing-library/react";
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
import { createEmptyProjection } from "./test/projectFixtures";

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
  exportPreview: async () => ({
    outputPath: "C:\\Temp\\Album-Horizonte_001.png",
    widthPx: 600,
    heightPx: 300,
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
