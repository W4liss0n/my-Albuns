import React, { useEffect, useLayoutEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import { OpeningExternalCopyDialog } from "./OpeningExternalCopyDialog";
import { OpeningRecoveryDialog } from "./OpeningRecoveryDialog";
import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import {
  resolveOpeningExternalCopy,
  resolveOpeningRecovery,
} from "../platform/tauriOpeningDialogControls";
import { dismissOwnedWindow } from "../platform/tauriOwnedDialogControls";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import { subscribeOpeningImageProgress } from "../platform/tauriOpeningImageProgress";
import type { StartupImageProgress } from "../contracts/generated/StartupImageProgress";
import { OpeningProgressDialog } from "./OpeningProgressDialog";
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

function openedFromLoadingOwner() {
  return window.sessionStorage.getItem(OPENING_OWNER_MARKER) === "loading";
}

function DialogContent() {
  const windowControls = useWindowControls();
  const kind = parameters.get("kind");
  const [imageProgress, setImageProgress] = useState<StartupImageProgress | null>(() => {
    const completedFiles = Number(parameters.get("imageCompleted"));
    const totalFiles = Number(parameters.get("imageTotal"));
    return Number.isSafeInteger(completedFiles) && Number.isSafeInteger(totalFiles) &&
      totalFiles > 0 && completedFiles >= 0 && completedFiles <= totalFiles
      ? { completedFiles, totalFiles } : null;
  });
  useEffect(() => {
    if (!["opening-project", "creating-project", "project-recovery", "external-copy"].includes(kind ?? "")) return;
    const subscription = subscribeOpeningImageProgress(setImageProgress);
    void subscription.ready.catch(() => undefined);
    return subscription.dispose;
  }, [kind]);
  useLayoutEffect(() => {
    if (kind === "opening-project") window.sessionStorage.setItem(OPENING_OWNER_MARKER, "loading");
  }, [kind]);

  if (imageProgress) {
    return <OpeningProgressDialog creating={kind === "creating-project"} images={imageProgress} />;
  }

  const closeDialog = () => {
    void Promise.resolve(windowControls.close()).catch(() => undefined);
  };

  if (kind === "processing-images") {
    return <ProgressDialog title="Processando imagens" progress={{
      kind: "determinate", completed: 0, total: 1,
    }} />;
  }

  if (kind === "creating-project") {
    return <OpeningProgressDialog creating />;
  }

  if (kind === "project-failure") {
    const title = parameter("title", "Não foi possível abrir o projeto");
    const message = parameter(
      "message",
      "O MyAlbuns encontrou um problema ao preparar a janela do projeto.",
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
    return (
      <OpeningRecoveryDialog
        attemptId={parameter("attemptId", "")}
        openedFromLoadingOwner={openedFromLoadingOwner()}
        resolveOpeningRecovery={resolveOpeningRecovery}
      />
    );
  }

  if (kind === "external-copy") {
    return (
      <OpeningExternalCopyDialog
        attemptId={parameter("attemptId", "")}
        openedFromLoadingOwner={openedFromLoadingOwner()}
        resolveOpeningExternalCopy={resolveOpeningExternalCopy}
      />
    );
  }

  return <OpeningProgressDialog />;
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
