import React, { useLayoutEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import type { ProjectRecoveryDecision } from "../application/projectPorts";
import { ProjectRecoveryDialog } from "../components/ProjectRecoveryDialog";
import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import { resolveOpeningRecovery } from "../platform/tauriOpeningDialogControls";
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
const OPENING_OWNER_MARKER = "myalbuns:opening-project-owner";

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
    const title = parameter("title", "Não foi possível abrir o Projeto");
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
        title={title}
        tone="error"
      />
    );
  }

  if (kind === "project-recovery") {
    return <OpeningRecoveryDialog />;
  }

  return <OpeningProgressDialog />;
}

function OpeningProgressDialog() {
  useLayoutEffect(() => {
    window.sessionStorage.setItem(OPENING_OWNER_MARKER, "loading");
  }, []);

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

function OpeningRecoveryDialog() {
  const attemptId = parameter("attemptId", "");
  const openedFromLoadingOwner =
    window.sessionStorage.getItem(OPENING_OWNER_MARKER) === "loading";
  const [state, setState] = useState<
    "available" | "confirmDiscard" | "resolving"
  >("available");
  const [error, setError] = useState<string | null>(null);

  const resolve = async (decision: ProjectRecoveryDecision) => {
    if (!attemptId || state === "resolving") return;
    setError(null);
    setState("resolving");
    try {
      await resolveOpeningRecovery(attemptId, decision);
    } catch (reason: unknown) {
      setError(
        reason instanceof Error
          ? reason.message
          : "Não foi possível concluir a escolha de Recuperação.",
      );
      setState(
        decision === "discardCheckpointAndOpenLastSaved"
          ? "confirmDiscard"
          : "available",
      );
    }
  };

  return (
    <div data-opening-owner-transition={String(openedFromLoadingOwner)}>
      <ProjectRecoveryDialog
        error={
          attemptId
            ? error
            : "A tentativa de abertura não está mais disponível."
        }
        onBack={() => {
          setError(null);
          setState("available");
        }}
        onDefer={() => void resolve("nowNot")}
        onDiscard={() =>
          void resolve("discardCheckpointAndOpenLastSaved")
        }
        onRecover={() => void resolve("reopenAndRecover")}
        onRequestDiscard={() => setState("confirmDiscard")}
        state={state}
      />
    </div>
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
