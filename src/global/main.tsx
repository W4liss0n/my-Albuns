import React from "react";
import ReactDOM from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SettingsWindow } from "../settings/SettingsWindow";
import { tauriPhotoshopPort, tauriPhotoshopSettingsPort } from "../platform/tauriPhotoshopPort";
import { tauriCacheSettingsPort } from "../platform/tauriCacheSettingsPort";
import type { SettingsSection } from "../application/photoshop";

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
const parameters = new URLSearchParams(window.location.search);
const settingsWindow = parameters.get("surface") === "settings";
const closeSettings = () => { void invoke("close_application_settings"); };
const onSettingsSection = (listener: (section: SettingsSection) => void) => listen<SettingsSection>("myalbuns://settings-section", (event) => {
  if (event.payload === "performance" || event.payload === "photoshop") listener(event.payload);
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WindowControlsProvider controls={settingsWindow ? { ...tauriWindowControls, close: closeSettings } : tauriWindowControls}>
      {settingsWindow ? <SettingsWindow photoshopPort={tauriPhotoshopSettingsPort} cachePort={tauriCacheSettingsPort}
        close={closeSettings} initialSection={parameters.get("section") === "photoshop" ? "photoshop" : "performance"}
        onSectionRequest={onSettingsSection} /> : <GlobalShell
        onOpenSettings={() => tauriPhotoshopPort.openSettings("performance")}
        failureDialogPort={tauriProjectFailureDialogPort}
        graphicsDiagnostic={graphicsDiagnostic}
        newProjectPort={tauriNewProjectPort}
        projectPort={tauriGlobalProjectPort}
      />}
    </WindowControlsProvider>
  </React.StrictMode>,
);
