import { render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import App from "./App";
import {
  type LogEvent,
  type Logger,
  silentLogger,
} from "./application/logging";
import type { ProjectBridge } from "./domain/project";
import { createEmptyProjection } from "./test/projectFixtures";

const projection = createEmptyProjection();

const bridge: ProjectBridge = {
  load: async () => projection,
  apply: async () => projection,
  undo: async () => projection,
  redo: async () => projection,
  exportPreview: async () => ({
    outputPath: "C:\\Temp\\Album-Horizonte_001.png",
    widthPx: 600,
    heightPx: 300,
  }),
};

test("keeps diagnostics available when hardware WebGL2 is unavailable", async () => {
  render(
    <App
      bridge={bridge}
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
});

test("opens the Project in the real workspace when hardware WebGL2 is available", async () => {
  const logEvents: LogEvent[] = [];
  const logger: Logger = {
    write: (event) => logEvents.push(event),
  };
  const load = vi.fn(async (_operationId: string) => projection);
  render(
    <App
      bridge={{ ...bridge, load }}
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
