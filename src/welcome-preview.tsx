import React from "react";
import ReactDOM from "react-dom/client";

import type {
  GlobalProjectPort,
  ProjectFailureDialogPort,
} from "./global/application/globalProjectPort";
import { GlobalShell } from "./global/GlobalShell";
import { createNewProjectPortStub } from "./global/testing/newProjectPortStub";
import "./ui/theme.css";
import "./ui/ui.css";
import "./global/GlobalShell.css";

const projectPort: GlobalProjectPort = {
  completeGraphicsGate: async () => null,
  openProject: async () => {
    if (new URLSearchParams(window.location.search).has("progress")) {
      await new Promise<never>(() => undefined);
    }
    return { status: "cancelled" };
  },
  listRecentProjects: async () => [
    { id: "p1", name: "Formatura Medicina 2026 — Turma B" },
    { id: "p2", name: "Casamento Marina & Téo" },
    { id: "p3", name: "Ensaio Helena — 6 meses" },
    { id: "p4", name: "15 anos Beatriz" },
    { id: "p5", name: "Corporativo Vetra — relatório anual" },
    { id: "p6", name: "Batizado Antônio" },
    { id: "p7", name: "Retrospectiva Estúdio 2025" },
  ],
  openRecentProject: async () => ({ status: "cancelled" }),
  startupOpenFailure: async () => null,
};

const failureDialogPort: ProjectFailureDialogPort = {
  present: async () => undefined,
};

const newProjectPort = createNewProjectPortStub();
const previewParameters = new URLSearchParams(window.location.search);

const supportedGraphics = {
  supported: true,
  renderer: "NVIDIA GeForce RTX",
  reason: "WebGL2 acelerado por hardware confirmado.",
  limits: {
    maxTextureSizePx: 16_384,
    maxRenderbufferSizePx: 16_384,
    maxTextureImageUnits: 16,
  },
} as const;

const unavailableGraphics = {
  supported: false,
  code: "hardware_unconfirmed",
  renderer: "Microsoft Basic Render Driver",
  reason: "A aceleração WebGL2 por hardware não pôde ser confirmada.",
  limits: null,
} as const;

const graphicsDiagnostic =
  previewParameters.get("graphics") === "unsupported"
    ? unavailableGraphics
    : supportedGraphics;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <GlobalShell
      failureDialogPort={failureDialogPort}
      graphicsDiagnostic={graphicsDiagnostic}
      newProjectPort={newProjectPort}
      projectPort={projectPort}
    />
  </React.StrictMode>,
);
