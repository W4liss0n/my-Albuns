import React from "react";
import ReactDOM from "react-dom/client";

import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import {
  parseInitialProjectDialogPresentation,
  parseInitialProjectDialogPreviewState,
} from "../platform/projectDialogContract";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { ProjectDialogApplication } from "./ProjectDialogApplication";
import { tauriProjectDialogClient } from "./platform/tauriProjectDialogClient";
import "../ui/theme.css";
import "../ui/ui.css";

installDesktopWebViewPolicy(document);

const initialPresentation = parseInitialProjectDialogPresentation(
  window.location.search,
);
const previewState = parseInitialProjectDialogPreviewState(
  window.location.search,
);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    {previewState && !initialPresentation ? (
      <ProjectDialogApplication
        mode="preview"
        state={previewState}
        windowControls={tauriWindowControls}
      />
    ) : (
      <ProjectDialogApplication
        client={tauriProjectDialogClient}
        initialPresentation={initialPresentation}
        mode="owned"
        windowControls={tauriWindowControls}
      />
    )}
  </React.StrictMode>,
);
