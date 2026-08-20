import React, { useEffect, useMemo, useState } from "react";
import ReactDOM from "react-dom/client";

import type {
  ProjectDialogAction,
  ProjectDialogState,
} from "../application/projectDialogPort";
import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import {
  MessageDialog,
  OwnedWindowShell,
  WindowControlsProvider,
  type WindowControls,
} from "../ui";
import type { ProjectDialogClient } from "./application/projectDialogClient";
import { parseInitialProjectDialogState } from "./application/projectDialogState";
import { ProjectDialogView } from "./ProjectDialogView";
import { tauriProjectDialogClient } from "./platform/tauriProjectDialogClient";
import "../ui/theme.css";
import "../ui/ui.css";

function defaultCloseAction(
  state: ProjectDialogState,
): ProjectDialogAction | null {
  switch (state.kind) {
    case "albumInformationConfirmation":
      return state.busy ? null : "cancelAlbumInformation";
    case "projectCloseConfirmation":
      return state.busy ? null : "cancelProjectClose";
    case "projectCloseFailure":
      return "dismissProjectCloseFailure";
    case "exportProgress":
      return state.cancellable && !state.cancelRequested
        ? "cancelExport"
        : null;
    case "exportFailure":
      return "dismissExport";
  }
}

function ProjectDialogApplication({
  client,
  initialState,
}: {
  client: ProjectDialogClient;
  initialState: ProjectDialogState | null;
}) {
  const [state, setState] = useState(initialState);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void client.onState((nextState) => {
      if (active) setState(nextState);
    }).then((dispose) => {
      if (active) unlisten = dispose;
      else dispose();
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [client]);

  const closeAction = state ? defaultCloseAction(state) : null;
  const controls = useMemo<WindowControls>(
    () => ({
      ...tauriWindowControls,
      close: closeAction
        ? () => client.submit(closeAction)
        : tauriWindowControls.close,
    }),
    [client, closeAction],
  );
  const submit = (action: ProjectDialogAction) => {
    void client.submit(action).catch(() => undefined);
  };

  return (
    <WindowControlsProvider controls={controls}>
      <OwnedWindowShell controls={closeAction ? "close" : "none"}>
        {state ? (
          <ProjectDialogView onAction={submit} state={state} />
        ) : (
          <MessageDialog
            description="O estado desta janela não pôde ser carregado."
            secondaryAction={{
              label: "Fechar",
              onClick: () => void tauriWindowControls.close(),
            }}
            title="Não foi possível abrir o diálogo"
            tone="error"
          />
        )}
      </OwnedWindowShell>
    </WindowControlsProvider>
  );
}

installDesktopWebViewPolicy(document);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <ProjectDialogApplication
      client={tauriProjectDialogClient}
      initialState={parseInitialProjectDialogState(window.location.search)}
    />
  </React.StrictMode>,
);
