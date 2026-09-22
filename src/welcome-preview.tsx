import React from "react";
import ReactDOM from "react-dom/client";

import type {
  GlobalProjectPort,
  ProjectFailureDialogPort,
} from "./global/application/globalProjectPort";
import { GlobalShell } from "./global/GlobalShell";
import { createNewProjectPortStub } from "./global/testing/newProjectPortStub";
import { welcomePreviewRecentProjects, welcomePreviewFirstSheet, welcomeDatesNow } from "./test/welcomePreviewFixtures";
import "./ui/theme.css";
import "./ui/ui.css";
import "./global/GlobalShell.css";

const previewParameters = new URLSearchParams(window.location.search);
let previewProjects = welcomePreviewRecentProjects(previewParameters);

const projectPort: GlobalProjectPort = {
  onActivationTerminal: async () => () => undefined,
  completeGraphicsGate: async () => null,
  openProject: async () => {
    if (new URLSearchParams(window.location.search).has("progress")) {
      await new Promise<never>(() => undefined);
    }
    return { status: "cancelled" };
  },
  listRecentProjects: async () => previewProjects,
  setRecentProjectFavorite: async (id, favorite) => {
    previewProjects = previewProjects.map((project) => project.id === id
      ? { ...project, favorite } : project);
    return { status: "saved", projects: previewProjects };
  },
  firstRecentProjectSheet: async (id) => {
    if (previewParameters.get("recents") === "loading") {
      return new Promise<never>(() => undefined);
    }
    return welcomePreviewFirstSheet(previewParameters, id);
  },
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
  reason: "A aceleração gráfica está disponível.",
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
  reason: "Não foi possível confirmar a aceleração gráfica necessária para abrir o editor.",
  limits: null,
} as const;

const graphicsDiagnostic =
  previewParameters.get("graphics") === "unsupported"
    ? unavailableGraphics
    : supportedGraphics;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <GlobalShell
      initialSurface={previewParameters.get("surface") === "newProject" ? "newProject" : "welcome"}
      recentProjectsNow={["dates", "long-names", "favorites", "favorites-only", "favorites-long-names"].includes(previewParameters.get("recents") ?? "")
        ? welcomeDatesNow : undefined}
      onOpenBatch={async () => { window.location.href = "/batch-export-preview.html?scenario=configuration"; }}
      onOpenSettings={previewParameters.get("graphics") === "unsupported" ? undefined : async () => { window.location.href = "/settings-preview.html?section=performance"; }}
      failureDialogPort={failureDialogPort}
      graphicsDiagnostic={graphicsDiagnostic}
      newProjectPort={newProjectPort}
      projectPort={projectPort}
    />
  </React.StrictMode>,
);
