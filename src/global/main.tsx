import React from "react";
import ReactDOM from "react-dom/client";

import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { GlobalShell } from "./GlobalShell";
import "./GlobalShell.css";
import { tauriGlobalProjectPort } from "./platform/tauriGlobalProjectPort";

installDesktopWebViewPolicy(document);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <GlobalShell projectPort={tauriGlobalProjectPort} />
  </React.StrictMode>,
);
