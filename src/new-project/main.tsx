import React, { useMemo } from "react";
import ReactDOM from "react-dom/client";

import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { NewProjectFlow } from "../global/NewProjectFlow";
import { tauriNewProjectPort } from "../global/platform/tauriNewProjectPort";
import {
  OwnedWindowShell,
  WindowControlsProvider,
  type WindowControls,
} from "../ui";
import "../ui/theme.css";
import "../ui/ui.css";
import "../global/NewProjectFlow.css";

function NewProjectApplication() {
  const closeWindow = () => tauriNewProjectPort.closeWindow();
  const controls = useMemo<WindowControls>(
    () => ({
      ...tauriWindowControls,
      close: closeWindow,
    }),
    [],
  );

  return (
    <WindowControlsProvider controls={controls}>
      <OwnedWindowShell controls="close" status="Novo Projeto">
        <NewProjectFlow
          onCancel={() => void closeWindow()}
          onChooseDecorative={() =>
            tauriNewProjectPort.chooseProvisionalDecorative()
          }
          onCreate={(configuration) =>
            tauriNewProjectPort.createProject(configuration)
          }
          onReleaseDecorative={(selectionId) =>
            tauriNewProjectPort.releaseProvisionalDecorative(selectionId)
          }
          onValidate={(configuration) =>
            tauriNewProjectPort.validateProjectConfiguration(configuration)
          }
        />
      </OwnedWindowShell>
    </WindowControlsProvider>
  );
}

installDesktopWebViewPolicy(document);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <NewProjectApplication />
  </React.StrictMode>,
);
