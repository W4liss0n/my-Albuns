import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { probeGraphics } from "./platform/graphics";
import { tauriProjectBridge } from "./platform/tauriProjectBridge";
import { tauriTopologyBenchmarkBridge } from "./platform/tauriTopologyBenchmarkBridge";
import { tauriLogger } from "./platform/tauriLogger";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App
      bridge={tauriProjectBridge}
      topologyBenchmarkBridge={tauriTopologyBenchmarkBridge}
      graphicsProbe={probeGraphics}
      logger={tauriLogger}
    />
  </React.StrictMode>,
);
