import React from "react";
import { BatchExportWindow } from "../batch-export/BatchExportWindow";
import { BatchProgressWindow } from "../batch-export/BatchProgressWindow";
import { openBatchExport, tauriBatchExportPort } from "../platform/tauriBatchExportPort";
import ReactDOM from "react-dom/client";
import { closeSettings, onSettingsSection } from "../platform/tauriSettingsWindow";
import { SettingsWindow } from "../settings/SettingsWindow";
import { tauriPhotoshopPort, tauriPhotoshopSettingsPort } from "../platform/tauriPhotoshopPort";
import { tauriCacheSettingsPort } from "../platform/tauriCacheSettingsPort";

import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { probeGraphics } from "../platform/graphics";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { WindowControlsProvider } from "../ui";
import { onNewProjectRequest } from "./platform/tauriNewProjectRequest";
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
const batchWindow = parameters.get("surface") === "batchExport";
const batchProgress = parameters.get("surface") === "batchProgress";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <WindowControlsProvider controls={settingsWindow ? { ...tauriWindowControls, close: closeSettings }
      : batchWindow ? { ...tauriWindowControls, close: tauriBatchExportPort.close } : tauriWindowControls}>
      {batchWindow ? <BatchExportWindow port={tauriBatchExportPort} />
      : batchProgress ? <BatchProgressWindow port={tauriBatchExportPort} />
      : settingsWindow ? <SettingsWindow photoshopPort={tauriPhotoshopSettingsPort} cachePort={tauriCacheSettingsPort}
        close={closeSettings} initialSection={parameters.get("section") === "photoshop" ? "photoshop" : "performance"}
        onSectionRequest={onSettingsSection} /> : <GlobalShell
        initialSurface={parameters.get("surface") === "newProject" ? "newProject" : "welcome"}
        onNewProjectRequest={onNewProjectRequest}
        onOpenSettings={() => tauriPhotoshopPort.openSettings("performance")}
        onOpenBatch={openBatchExport}
        failureDialogPort={tauriProjectFailureDialogPort}
        graphicsDiagnostic={graphicsDiagnostic}
        newProjectPort={tauriNewProjectPort}
        projectPort={tauriGlobalProjectPort}
      />}
    </WindowControlsProvider>
  </React.StrictMode>,
);

if (!settingsWindow && !batchWindow && !batchProgress) {
  void tauriBatchExportPort.recoveries().then(pending => {
    if (pending.length > 0) return openBatchExport();
  }).catch(() => undefined);
}
