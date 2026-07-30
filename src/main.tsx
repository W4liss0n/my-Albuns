import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { probeGraphics } from "./platform/graphics";
import {
  tauriExportPort,
  tauriMediaPreviewPort,
  tauriProjectSessionPort,
} from "./platform/tauriProjectPorts";
import { tauriTopologyBenchmarkBridge } from "./platform/tauriTopologyBenchmarkBridge";
import { tauriLogger } from "./platform/tauriLogger";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App
      exportPort={tauriExportPort}
      mediaPreviewPort={tauriMediaPreviewPort}
      projectSessionPort={tauriProjectSessionPort}
      topologyBenchmarkBridge={tauriTopologyBenchmarkBridge}
      graphicsProbe={probeGraphics}
      logger={tauriLogger}
    />
  </React.StrictMode>,
);
