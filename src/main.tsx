import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import {
  probeCanvasGraphics,
  probeGraphics,
} from "./platform/graphics";
import {
  tauriExportPipelinePort,
  tauriMediaPreviewPort,
  tauriProjectStartupPort,
  tauriProjectCorePort,
  tauriWorkspacePreferencesPort,
} from "./platform/tauriProjectPorts";
import { installDesktopWebViewPolicy } from "./platform/desktopWebViewPolicy";
import { tauriLogger } from "./platform/tauriLogger";
import { tauriProjectWindowPort } from "./platform/tauriProjectWindowPort";
import { tauriProjectDialogPort } from "./platform/tauriProjectDialogPort";
import { tauriWindowControls } from "./platform/tauriWindowControls";
import { WindowControlsProvider } from "./ui";

installDesktopWebViewPolicy(document);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WindowControlsProvider controls={tauriWindowControls}>
      <App
        exportPipelinePort={tauriExportPipelinePort}
        mediaPreviewPort={tauriMediaPreviewPort}
        projectStartupPort={tauriProjectStartupPort}
        projectCorePort={tauriProjectCorePort}
        projectDialogPort={tauriProjectDialogPort}
        projectWindowPort={tauriProjectWindowPort}
        graphicsProbe={probeGraphics}
        canvasGraphicsDiagnosticProbe={probeCanvasGraphics}
        logger={tauriLogger}
        workspacePreferencesPort={tauriWorkspacePreferencesPort}
      />
    </WindowControlsProvider>
  </React.StrictMode>,
);
