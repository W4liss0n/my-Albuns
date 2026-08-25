import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import type {
  GraphicsDiagnostic,
  GraphicsProbe,
} from "./application/graphics";
import { silentLogger } from "./application/logging";
import type {
  ExportPort,
  MediaPreviewPort,
  ProjectSessionPort,
  ProjectStartupPort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { ProjectDialogPort } from "./application/projectDialogPort";
import {
  applyWorkspacePreferenceChange,
  createWorkspacePreferences,
  type WorkspacePreferencesPort,
} from "./application/workspacePreferences";
import type { CanvasGraphicsDiagnosticProbe } from "./components/canvasGraphicsDiagnosticProbeContext";
import type { EditorProjection } from "./domain/project";
import { createTwoSheetProjection } from "./test/projectFixtures";
import "./ui/theme.css";
import "./ui/ui.css";

const previewParameters = new URLSearchParams(window.location.search);
const frameContext = previewParameters.get("frame");
let projection = createPreviewProjection(frameContext);

const projectSessionPort: ProjectSessionPort = {
  load: async () => projection,
  validateAlbumInformation: async () => ({
    errors: [],
    impact: {
      heightPx: 3_543,
      pageWidthPx: 3_543,
      sheetWidthPx: 7_087,
    },
  }),
  apply: async () => projection,
  undo: async () => projection,
  redo: async () => projection,
  save: async () => ({
    outcome: { kind: "saved", revision: projection.state.revision },
    projection,
  }),
};

const mediaPreviewPort: MediaPreviewPort = {
  prepareMediaPreviews: async () => null,
  onMediaChanged: async () => () => undefined,
};

const exportPort: ExportPort = {
  startSheet: () => ({
    completion: Promise.resolve({
      status: "completed",
      result: { heightPx: 3_543, widthPx: 7_087 },
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
  dismiss: async () => undefined,
  onAction: async () => () => undefined,
  present: async () => undefined,
};

const projectStartupPort: ProjectStartupPort = {
  confirmUiReady: async () => undefined,
};

const supportedGraphics: Extract<GraphicsDiagnostic, { supported: true }> = {
  supported: true,
  renderer: "integrated-acceptance-preview",
  reason: "Prévia integrada determinística.",
  limits: {
    maxTextureImageUnits: 16,
    maxRenderbufferSizePx: 16_384,
    maxTextureSizePx: 16_384,
  },
};

const unavailableGraphics: Extract<GraphicsDiagnostic, { supported: false }> = {
  supported: false,
  code: "hardware_unconfirmed",
  renderer: "Microsoft Basic Render Driver",
  reason: "A aceleração WebGL2 por hardware não pôde ser confirmada.",
  limits: null,
};

const graphicsDiagnostic =
  previewParameters.get("graphics") === "unsupported"
    ? unavailableGraphics
    : supportedGraphics;
const graphicsProbe: GraphicsProbe = () => graphicsDiagnostic;

const canvasGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe = () => ({
  ...supportedGraphics,
});

const workspacePreferencesPort = createPreviewWorkspacePreferencesPort(
  previewParameters.get("layout"),
);

const appProps = {
  canvasGraphicsDiagnosticProbe,
  exportPort,
  graphicsProbe,
  logger: silentLogger,
  mediaPreviewPort,
  projectDialogPort,
  projectSessionPort,
  projectStartupPort,
  projectWindowPort,
};

const root = document.getElementById("root");
if (root) {
  ReactDOM.createRoot(root).render(
    <React.StrictMode>
      {workspacePreferencesPort ? (
        <App {...appProps} workspacePreferencesPort={workspacePreferencesPort} />
      ) : (
        <App {...appProps} workspacePreferencesMode="memory" />
      )}
    </React.StrictMode>,
  );
}

function createPreviewProjection(frameMode: string | null): EditorProjection {
  const preview = structuredClone(createTwoSheetProjection());
  if (frameMode !== "photo" && frameMode !== "empty") return preview;

  const sheet = preview.state.album.sheets[0];
  const frame = sheet?.frames[0];
  const composedSheet = preview.composition.sheets[0];
  const composedFrame = composedSheet?.frames[0];
  if (!sheet || !frame || !composedSheet || !composedFrame) return preview;

  const fullSheetRect = {
    x: 0,
    y: 0,
    width: sheet.widthUm,
    height: sheet.heightUm,
  };
  frame.rect = fullSheetRect;
  composedFrame.clipRect = fullSheetRect;
  if (frameMode === "empty") {
    frame.photo = null;
    composedFrame.photo = null;
  }
  return preview;
}

function createPreviewWorkspacePreferencesPort(
  layout: string | null,
): WorkspacePreferencesPort | null {
  if (layout !== "persisted" && layout !== "collapsed") return null;
  let preferences = createWorkspacePreferences({
    workspacePanels:
      layout === "persisted"
        ? {
            inspector: { size: 420, visible: true },
            media: { size: 140, visible: true },
          }
        : {
            inspector: { size: 310, visible: false },
            media: { size: 202, visible: false },
          },
  });
  return {
    load: async () => preferences,
    update: async (change) => {
      preferences = applyWorkspacePreferenceChange(preferences, change);
      return preferences;
    },
  };
}
