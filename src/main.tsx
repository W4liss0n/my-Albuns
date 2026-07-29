import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { probeGraphics } from "./platform/graphics";
import { tauriProjectBridge } from "./platform/tauriProjectBridge";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App
      bridge={tauriProjectBridge}
      graphicsProbe={probeGraphics}
    />
  </React.StrictMode>,
);
