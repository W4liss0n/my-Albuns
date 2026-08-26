import React from "react";
import ReactDOM from "react-dom/client";

import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { probeGraphics } from "../platform/graphics";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { WindowControlsProvider } from "../ui";
import { GlobalShell } from "./GlobalShell";
import "../ui/theme.css";
import "../ui/ui.css";
import "./GlobalShell.css";
import { tauriGlobalProjectPort } from "./platform/tauriGlobalProjectPort";
import { tauriNewProjectPort } from "./platform/tauriNewProjectPort";
import { tauriProjectFailureDialogPort } from "./platform/tauriProjectFailureDialogPort";

installDesktopWebViewPolicy(document);
const graphicsDiagnostic = probeGraphics();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WindowControlsProvider controls={tauriWindowControls}>
      <GlobalShell
        failureDialogPort={tauriProjectFailureDialogPort}
        graphicsDiagnostic={graphicsDiagnostic}
        newProjectPort={tauriNewProjectPort}
        projectPort={tauriGlobalProjectPort}
      />
    </WindowControlsProvider>
  </React.StrictMode>,
);
