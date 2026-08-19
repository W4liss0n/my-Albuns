import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  useState,
} from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogState,
} from "../application/projectDialogPort";
import type {
  ExportAttempt,
  ExportPort,
  ExportProgressEvent,
  ExportProgressStage,
} from "../application/projectPorts";
import { ActionButton, InlineNotice } from "../ui";
import "./ExportPreviewControl.css";

interface ExportPreviewControlProps {
  dialogPort: ProjectDialogPort;
  disabled?: boolean;
  exportPort: ExportPort;
  onActiveChange?(active: boolean): void;
  projectId: string;
  sheetId: string | null;
}

export interface ExportPreviewControlHandle {
  start(): void;
}

interface ExportNotification {
  kind: "error" | "success";
  message: string;
}

export const ExportPreviewControl = forwardRef<
  ExportPreviewControlHandle,
  ExportPreviewControlProps
>(function ExportPreviewControl(
  {
    dialogPort,
    disabled = false,
    exportPort,
    onActiveChange,
    projectId,
    sheetId,
  },
  ref,
) {
  const [phase, setPhase] = useState<
    "idle" | "starting" | "running" | "cancelled" | "failed"
  >("idle");
  const [notification, setNotification] =
    useState<ExportNotification | null>(null);
  const nextAttemptId = useRef(0);
  const currentAttemptId = useRef<number | null>(null);
  const startedAttemptId = useRef<number | null>(null);
  const activeAttempt = useRef<{
    attempt: ExportAttempt;
    cancelRequested: boolean;
    id: number;
  } | undefined>(undefined);
  const activeChangeListener = useRef<
    ExportPreviewControlProps["onActiveChange"]
  >(undefined);
  const interactionActive = useRef(false);
  const notificationTimer = useRef<number | undefined>(undefined);
  const lastDialogState = useRef<ProjectDialogState | undefined>(undefined);
  const dialogPresentationFailed = useRef(false);
  const dialogActionListener = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  useImperativeHandle(ref, () => ({ start: startExport }));

  dialogActionListener.current = (action) => {
    switch (action) {
      case "cancelExport":
        requestCancellation();
        break;
      case "retryExport":
        retryExport();
        break;
      case "dismissExport":
        dismissFeedback();
        break;
      default:
        break;
    }
  };

  useLayoutEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void dialogPort
      .onAction((action) => dialogActionListener.current(action))
      .then((dispose) => {
        if (active) {
          unlisten = dispose;
        } else {
          dispose();
        }
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [dialogPort]);

  useLayoutEffect(() => {
    setPhase("idle");
    clearNotification();
    lastDialogState.current = undefined;
    dialogPresentationFailed.current = false;
    void dialogPort.dismiss().catch(() => undefined);

    return () => {
      retireActiveAttempt();
      clearNotificationTimer();
      lastDialogState.current = undefined;
      void dialogPort.dismiss().catch(() => undefined);
    };
  }, [dialogPort, projectId]);

  function startExport() {
    if (disabled || !sheetId || currentAttemptId.current !== null) {
      return;
    }

    const attemptId = ++nextAttemptId.current;
    currentAttemptId.current = attemptId;
    beginInteraction();
    setPhase("starting");
    clearNotification();
    dialogPresentationFailed.current = false;

    let attempt: ExportAttempt;
    try {
      attempt = exportPort.startSheet(sheetId, (event) => {
        if (currentAttemptId.current !== attemptId) {
          return;
        }

        if (event.event === "started") {
          startedAttemptId.current = attemptId;
          setPhase("running");
          presentDialog({
            cancelRequested: false,
            cancellable: event.cancellable,
            kind: "exportProgress",
            progress: {
              kind: "indeterminate",
              note: "sem estimativa de tempo",
              status: "Iniciando a Exportação",
            },
          });
          return;
        }

        presentDialog(progressDialogState(event));
      });
    } catch (error: unknown) {
      finishAttemptWithFailure(attemptId, error);
      return;
    }

    if (currentAttemptId.current !== attemptId) {
      void attempt.cancel().catch(() => undefined);
      return;
    }

    activeAttempt.current = {
      attempt,
      cancelRequested: false,
      id: attemptId,
    };
    void attempt.completion.then(
      (outcome) => {
        const finished = finishActiveAttempt(attemptId);
        if (!finished) return;

        if (outcome.status === "cancelled") {
          if (finished.started) {
            if (dialogPresentationFailed.current) {
              setPhase("idle");
              lastDialogState.current = undefined;
              endInteraction();
              return;
            }
            setPhase("cancelled");
            presentDialog({
              cancelled: true,
              kind: "exportFailure",
              message: "A Exportação foi cancelada.",
              retryDisabled: false,
            });
          } else {
            setPhase("idle");
            endInteraction();
          }
          return;
        }

        setPhase("idle");
        endInteraction();
        lastDialogState.current = undefined;
        void dialogPort.dismiss().catch(() => undefined);
        showNotification({
          kind: "success",
          message: "Exportação concluída",
        });
      },
      (error: unknown) => finishAttemptWithFailure(attemptId, error),
    );
  }

  function retryExport() {
    if (phase !== "cancelled" && phase !== "failed") return;
    const current = lastDialogState.current;
    if (current?.kind === "exportFailure") {
      presentDialog({ ...current, retryDisabled: true });
    }
    startExport();
  }

  function requestCancellation() {
    const current = activeAttempt.current;
    if (!current || current.cancelRequested) return;

    current.cancelRequested = true;
    const dialogState = lastDialogState.current;
    if (dialogState?.kind === "exportProgress") {
      presentDialog({ ...dialogState, cancelRequested: true });
    }
    void current.attempt.cancel().catch(() => undefined);
  }

  function dismissFeedback() {
    if (phase !== "cancelled" && phase !== "failed") return;
    setPhase("idle");
    lastDialogState.current = undefined;
    void dialogPort.dismiss().catch(() => undefined);
    endInteraction();
  }

  function presentDialog(state: ProjectDialogState) {
    lastDialogState.current = state;
    void dialogPort.present(state).catch((error: unknown) => {
      if (dialogPresentationFailed.current) return;
      dialogPresentationFailed.current = true;
      showNotification({ kind: "error", message: messageFromError(error) });
      const current = activeAttempt.current;
      if (current && !current.cancelRequested) {
        current.cancelRequested = true;
        void current.attempt.cancel().catch(() => undefined);
      } else if (!current) {
        setPhase("idle");
        lastDialogState.current = undefined;
        endInteraction();
      }
    });
  }

  function finishActiveAttempt(attemptId: number) {
    if (currentAttemptId.current !== attemptId) return false;

    currentAttemptId.current = null;
    activeAttempt.current = undefined;
    const started = startedAttemptId.current === attemptId;
    startedAttemptId.current = null;
    return { started };
  }

  function finishAttemptWithFailure(attemptId: number, error: unknown) {
    const finished = finishActiveAttempt(attemptId);
    if (!finished) return;

    const message = messageFromError(error);
    if (finished.started) {
      if (dialogPresentationFailed.current) {
        setPhase("idle");
        lastDialogState.current = undefined;
        endInteraction();
        return;
      }
      setPhase("failed");
      presentDialog({
        cancelled: false,
        kind: "exportFailure",
        message,
        retryDisabled: false,
      });
      return;
    }

    setPhase("idle");
    endInteraction();
    showNotification({ kind: "error", message });
  }

  function retireActiveAttempt() {
    const attemptId = currentAttemptId.current;
    if (attemptId !== null) {
      const current = activeAttempt.current;
      if (current?.id === attemptId && !current.cancelRequested) {
        current.cancelRequested = true;
        void current.attempt.cancel().catch(() => undefined);
      }
      finishActiveAttempt(attemptId);
    }
    endInteraction();
  }

  function beginInteraction() {
    if (interactionActive.current) return;
    interactionActive.current = true;
    activeChangeListener.current = onActiveChange;
    onActiveChange?.(true);
  }

  function endInteraction() {
    if (!interactionActive.current) return;
    interactionActive.current = false;
    const notify = activeChangeListener.current;
    activeChangeListener.current = undefined;
    notify?.(false);
  }

  function clearNotificationTimer() {
    if (notificationTimer.current !== undefined) {
      window.clearTimeout(notificationTimer.current);
      notificationTimer.current = undefined;
    }
  }

  function clearNotification() {
    clearNotificationTimer();
    setNotification(null);
  }

  function showNotification(nextNotification: ExportNotification) {
    clearNotificationTimer();
    setNotification(nextNotification);
    notificationTimer.current = window.setTimeout(() => {
      notificationTimer.current = undefined;
      setNotification(null);
    }, nextNotification.kind === "success" ? 3_000 : 6_000);
  }

  return (
    <div className="export-preview-control">
      <ActionButton
        aria-label="Exportar Lâmina"
        className="export-preview-trigger"
        disabled={disabled || !sheetId || phase !== "idle"}
        onClick={startExport}
        variant="primary"
      >
        Exportar
      </ActionButton>

      {notification ? (
        <InlineNotice
          className="export-preview-notification"
          floating
          role={notification.kind === "error" ? "alert" : "status"}
          tone={notification.kind}
        >
          <p>{notification.message}</p>
        </InlineNotice>
      ) : null}
    </div>
  );
});

function progressDialogState(
  event: Extract<ExportProgressEvent, { event: "progress" }>,
): ProjectDialogState {
  const status = progressStageLabel(event.stage);
  return {
    cancelRequested: false,
    cancellable: event.cancellable,
    kind: "exportProgress",
    progress:
      event.units.kind === "measured"
        ? {
            completed: event.units.completedUnits,
            kind: "determinate",
            status,
            total: event.units.totalUnits,
          }
        : {
            kind: "indeterminate",
            note: "sem estimativa de tempo",
            status,
          },
  };
}

function messageFromError(error: unknown) {
  if (error instanceof Error) return error.message;
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof error.message === "string"
  ) {
    return error.message;
  }
  return "Não foi possível exportar a prova.";
}

function progressStageLabel(stage: ExportProgressStage) {
  switch (stage) {
    case "preparing":
      return "Preparando a prova";
    case "loading_sources":
      return "Carregando os originais";
    case "composing":
      return "Compondo a prova";
    case "encoding_output":
      return "Codificando a prova";
    case "verifying":
      return "Verificando a prova";
    case "publishing":
      return "Publicando a prova";
    case "completed":
      return "Finalizando a prova";
  }
}
