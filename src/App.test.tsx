import { render, screen, waitFor } from "@testing-library/react";
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

test("keeps diagnostics available when hardware WebGL2 is unavailable", async () => {
  const load = vi.fn(async () => projection);
  const prepareMediaPreviews = vi.fn(async () => null);
  render(
    <App
      exportPort={exportPort}
      mediaPreviewPort={{ prepareMediaPreviews }}
      projectSessionPort={{ ...projectSessionPort, load }}
      logger={silentLogger}
      graphicsProbe={() => ({
        supported: false,
        renderer: "indisponível",
        reason: "WebGL2 acelerado por hardware não foi confirmado.",
      })}
    />,
  );

  expect(
    await screen.findByRole("heading", {
      name: "Editor indisponível neste computador",
    }),
  ).toBeInTheDocument();
  expect(
    screen.getByText("WebGL2 acelerado por hardware não foi confirmado."),
  ).toBeInTheDocument();
  expect(screen.getByText("Diagnóstico gráfico")).toBeInTheDocument();
  await waitFor(() => expect(load).toHaveBeenCalledOnce());
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
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
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
      graphicsProbe={() => ({
        supported: true,
        renderer: "NVIDIA GeForce RTX",
        reason: "WebGL2 acelerado por hardware confirmado.",
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
