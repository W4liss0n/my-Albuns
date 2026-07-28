import { render, screen } from "@testing-library/react";
import { expect, test } from "vitest";

import App from "./App";
import type { EditorProjection, ProjectBridge } from "./domain/project";

const projection: EditorProjection = {
  state: {
    projectId: "project-spike-001",
    projectName: "Álbum Horizonte",
    revision: 0,
    savedRevision: 0,
    dirty: false,
    canUndo: false,
    canRedo: false,
    album: {
      sheets: [],
      media: [],
    },
  },
  composition: {
    sheets: [],
  },
};

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
  render(
    <App
      bridge={bridge}
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
});
