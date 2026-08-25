import React from "react";
import ReactDOM from "react-dom/client";

import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { dismissOwnedWindow } from "../platform/tauriOwnedDialogControls";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import {
  MessageDialog,
  OwnedWindowShell,
  ProgressDialog,
  useWindowControls,
  WindowControlsProvider,
} from "../ui";
import "../ui/theme.css";
import "../ui/ui.css";

const parameters = new URLSearchParams(window.location.search);

function parameter(name: string, fallback: string) {
  const value = parameters.get(name)?.trim();
  return value ? value.slice(0, 800) : fallback;
}

function DialogContent() {
  const windowControls = useWindowControls();
  const kind = parameters.get("kind");

  const closeDialog = () => {
    void Promise.resolve(windowControls.close()).catch(() => undefined);
  };

  if (kind === "creating-project") {
    return (
      <ProgressDialog
        progress={{
          kind: "indeterminate",
          status: "Preparando a Janela do Projeto…",
        }}
        title="Criando Projeto"
      />
    );
  }

  if (kind === "project-failure") {
    const message = parameter(
      "message",
      "Não foi possível abrir este Projeto.",
    );
    const action = parameter("action", "Tente novamente.");

    return (
      <MessageDialog
        description={
          <>
            <p>{message}</p>
            <p>{action}</p>
          </>
        }
        secondaryAction={{ label: "Fechar", onClick: closeDialog }}
        title="Não foi possível abrir o Projeto"
        tone="error"
      />
    );
  }

  return (
    <ProgressDialog
      progress={{
        kind: "indeterminate",
        status: "Preparando a Janela do Projeto…",
      }}
      title="Abrindo Projeto"
    />
  );
}

function DialogWindow() {
  const kind = parameters.get("kind");
  const controls =
    kind === "project-failure"
      ? { ...tauriWindowControls, close: dismissOwnedWindow }
      : tauriWindowControls;

  return (
    <WindowControlsProvider controls={controls}>
      <OwnedWindowShell
        controls={kind === "project-failure" ? "close" : "none"}
      >
        <DialogContent />
      </OwnedWindowShell>
    </WindowControlsProvider>
  );
}

installDesktopWebViewPolicy(document);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <DialogWindow />
  </React.StrictMode>,
);
