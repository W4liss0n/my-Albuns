import React from "react";
import ReactDOM from "react-dom/client";

import type {
  GlobalProjectPort,
  ProjectFailureDialogPort,
} from "./global/application/globalProjectPort";
import { GlobalShell } from "./global/GlobalShell";
import { createNewProjectPortStub } from "./global/testing/newProjectPortStub";
import { welcomePreviewRecentProjects } from "./test/welcomePreviewFixtures";
import "./ui/theme.css";
import "./ui/ui.css";
import "./global/GlobalShell.css";

const previewParameters = new URLSearchParams(window.location.search);

const projectPort: GlobalProjectPort = {
  onActivationTerminal: async () => () => undefined,
  completeGraphicsGate: async () => null,
  openProject: async () => {
    if (new URLSearchParams(window.location.search).has("progress")) {
      await new Promise<never>(() => undefined);
    }
    return { status: "cancelled" };
  },
  listRecentProjects: async () =>
    welcomePreviewRecentProjects(previewParameters),
  openRecentProject: async () => ({ status: "cancelled" }),
  startupOpenFailure: async () => null,
};

const failureDialogPort: ProjectFailureDialogPort = {
  present: async () => undefined,
};

const newProjectPort = createNewProjectPortStub();

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
