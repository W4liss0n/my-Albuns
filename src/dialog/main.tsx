import React, { useEffect, useLayoutEffect, useState } from "react";
import ReactDOM from "react-dom/client";

import { OpeningExternalCopyDialog } from "./OpeningExternalCopyDialog";
import { OpeningRecoveryDialog } from "./OpeningRecoveryDialog";
import { ProjectFailureList } from "./ProjectFailureList";
import { parseProjectFailureDetails } from "../global/platform/projectLaunchBridge";
import { installDesktopWebViewPolicy } from "../platform/desktopWebViewPolicy";
import {
  resolveOpeningExternalCopy,
  resolveOpeningRecovery,
} from "../platform/tauriOpeningDialogControls";
import { dismissOwnedWindow } from "../platform/tauriOwnedDialogControls";
import { tauriWindowControls } from "../platform/tauriWindowControls";
import {
  parseOpeningProgress,
  subscribeOpeningProgress,
} from "../platform/tauriOpeningProgress";
import type { OpeningProgress } from "../contracts/generated/OpeningProgress";
import {
  OPENING_PROJECT_LIST_WIDTH,
  OpeningProgressDialog,
  OpeningProjectsDialog,
  openingProgressView,
  type OpeningProgressView,
} from "./OpeningProgressDialog";
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

/** Projects opened together that did not open, each with its own reason. */
function failedProjects() {
  const projects = parameters.get("projects");
  if (!projects) return null;
  try {
    return parseProjectFailureDetails(JSON.parse(projects));
  } catch {
    return null;
  }
}

function openedFromLoadingOwner() {
  return window.sessionStorage.getItem(OPENING_OWNER_MARKER) === "loading";
}

const openingKinds = ["opening-project", "creating-project", "project-recovery", "external-copy"];
const decisionKinds = ["project-recovery", "external-copy"];

/**
 * Preview pages describe the rows in the address: one Project's photos, or
 * several Projects as JSON.
 */
function initialOpeningProgress(): OpeningProgress | null {
  const projects = parameters.get("openingProjects");
  if (projects) {
    try {
      return parseOpeningProgress(JSON.parse(projects));
    } catch {
      return null;
    }
  }
  const completedFiles = Number(parameters.get("imageCompleted"));
  const totalFiles = Number(parameters.get("imageTotal"));
  return Number.isSafeInteger(completedFiles) && Number.isSafeInteger(totalFiles) &&
    totalFiles > 0 && completedFiles >= 0 && completedFiles <= totalFiles
    ? { projects: [{ name: "", state: "preparing", completedFiles, totalFiles }] }
    : null;
}

function useOpeningProgressView(kind: string | null): OpeningProgressView | null {
  const [progress, setProgress] = useState<OpeningProgress | null>(initialOpeningProgress);
  useEffect(() => {
    if (!openingKinds.includes(kind ?? "")) return;
    const subscription = subscribeOpeningProgress(setProgress);
    void subscription.ready.catch(() => undefined);
    return subscription.dispose;
  }, [kind]);
  return openingProgressView(progress, decisionKinds.includes(kind ?? ""));
}

function DialogContent({ opening }: { opening: OpeningProgressView | null }) {
  const windowControls = useWindowControls();
  const kind = parameters.get("kind");
  useLayoutEffect(() => {
    if (kind === "opening-project") window.sessionStorage.setItem(OPENING_OWNER_MARKER, "loading");
  }, [kind]);

  if (opening?.kind === "list") {
    return <OpeningProjectsDialog projects={opening.projects} />;
  }
  if (opening?.kind === "single") {
    return <OpeningProgressDialog creating={kind === "creating-project"} images={opening.images} />;
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
    const projects = failedProjects();

    return (
      <MessageDialog
        description={projects ? <ProjectFailureList projects={projects} /> : (
          <>
            <p>{message}</p>
            <p>{action}</p>
          </>
        )}
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
        projectName={parameters.get("projectName")?.trim() || null}
        resolveOpeningRecovery={resolveOpeningRecovery}
      />
    );
  }

  if (kind === "external-copy") {
    return (
      <OpeningExternalCopyDialog
        attemptId={parameter("attemptId", "")}
        openedFromLoadingOwner={openedFromLoadingOwner()}
        projectName={parameters.get("projectName")?.trim() || null}
        resolveOpeningExternalCopy={resolveOpeningExternalCopy}
      />
    );
  }

  return <OpeningProgressDialog />;
}

function DialogWindow() {
  const kind = parameters.get("kind");
  const opening = useOpeningProgressView(kind);
  const controls =
    kind === "project-failure"
      ? { ...tauriWindowControls, close: dismissOwnedWindow }
      : tauriWindowControls;

  return (
    <WindowControlsProvider controls={controls}>
      <OwnedWindowShell
        controls={kind === "project-failure" ? "close" : "none"}
        width={opening?.kind === "list" ? OPENING_PROJECT_LIST_WIDTH : undefined}
      >
        <DialogContent opening={opening} />
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
