import React from "react";
import ReactDOM from "react-dom/client";

import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { probeGraphics } from "../platform/graphics";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { WindowControlsProvider } from "../ui";
import { GlobalShell } from "./GlobalShell";
import "../App.css";
import "./GlobalShell.css";
import { tauriGlobalProjectPort } from "./platform/tauriGlobalProjectPort";

installDesktopWebViewPolicy(document);
const graphicsDiagnostic = probeGraphics();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WindowControlsProvider controls={tauriWindowControls}>
      <GlobalShell
        graphicsDiagnostic={graphicsDiagnostic}
        projectPort={tauriGlobalProjectPort}
      />
    </WindowControlsProvider>
  </React.StrictMode>,
);
