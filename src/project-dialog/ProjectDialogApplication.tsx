import { useEffect, useMemo, useState } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPresentation,
  ProjectDialogState,
} from "../application/projectDialogPort";
import {
  MessageDialog,
  OwnedWindowShell,
  WindowControlsProvider,
  type WindowControls,
} from "../ui";
import type { ProjectDialogClient } from "./application/projectDialogClient";
import { defaultProjectDialogCloseAction } from "./application/projectDialogLifecycle";
import { ProjectDialogView } from "./ProjectDialogView";

type ProjectDialogApplicationProps =
  | {
      client: ProjectDialogClient;
      initialPresentation: ProjectDialogPresentation | null;
      mode: "owned";
      windowControls: WindowControls;
    }
  | {
      mode: "preview";
      state: ProjectDialogState;
      windowControls: WindowControls;
    };

export function ProjectDialogApplication(
  props: ProjectDialogApplicationProps,
) {
  const client = props.mode === "owned" ? props.client : null;
  const windowControls = props.windowControls;
  const [presentation, setPresentation] =
    useState<ProjectDialogPresentation | null>(() =>
      props.mode === "owned" ? props.initialPresentation : null,
    );

  useEffect(() => {
    if (!client) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void client
      .onPresentation((nextPresentation) => {
        if (active) setPresentation(nextPresentation);
      })
      .then((dispose) => {
        if (active) unlisten = dispose;
        else dispose();
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [client]);

  const state = props.mode === "preview" ? props.state : presentation?.state;
  const sessionId = presentation?.sessionId ?? null;
  const closeAction = state
    ? defaultProjectDialogCloseAction(state)
    : null;
  const controls = useMemo<WindowControls>(
    () => ({
      ...windowControls,
      close:
        closeAction && client && sessionId
          ? () => client.submit(sessionId, closeAction)
          : windowControls.close,
    }),
    [client, closeAction, sessionId, windowControls],
  );
  const submit = (action: ProjectDialogAction) => {
    if (!client || !sessionId) return;
    void client.submit(sessionId, action).catch(() => undefined);
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
              onClick: () => void windowControls.close(),
            }}
            title="Não foi possível abrir o diálogo"
            tone="error"
          />
        )}
      </OwnedWindowShell>
    </WindowControlsProvider>
  );
}
