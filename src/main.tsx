import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import {
  probeCanvasGraphics,
  probeGraphics,
} from "./platform/graphics";
import {
  tauriExportPort,
  tauriMediaPreviewPort,
  tauriProjectSessionPort,
} from "./platform/tauriProjectPorts";
import { installDesktopWebViewPolicy } from "./platform/desktopWebViewPolicy";
import { tauriLogger } from "./platform/tauriLogger";
import { tauriProjectWindowPort } from "./platform/tauriProjectWindowPort";

installDesktopWebViewPolicy(document);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App
      exportPort={tauriExportPort}
      mediaPreviewPort={tauriMediaPreviewPort}
      projectSessionPort={tauriProjectSessionPort}
      projectWindowPort={tauriProjectWindowPort}
      graphicsProbe={probeGraphics}
      canvasGraphicsDiagnosticProbe={probeCanvasGraphics}
      logger={tauriLogger}
    />
  </React.StrictMode>,
);
