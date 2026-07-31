import type { ReactNode } from "react";
import { useLayoutEffect, useRef, useState } from "react";
import { Dialog, Modal, ModalOverlay } from "react-aria-components";

import type {
  ExportAttempt,
  ExportPort,
  ExportProgressEvent,
  ExportProgressStage,
} from "../application/projectPorts";
import "./ExportPreviewControl.css";

interface ExportPreviewControlProps {
  disabled?: boolean;
  exportPort: ExportPort;
  onActiveChange?(active: boolean): void;
  projectId: string;
}

interface ExportNotification {
  kind: "error" | "success";
  message: string;
}

export function ExportPreviewControl({
  disabled = false,
  exportPort,
  onActiveChange,
  projectId,
}: ExportPreviewControlProps) {
  const [phase, setPhase] = useState<
    "idle" | "starting" | "running" | "cancelled" | "failed"
  >("idle");
  const [progress, setProgress] = useState<
    Extract<ExportProgressEvent, { event: "progress" }> | undefined
  >();
  const [cancellable, setCancellable] = useState(false);
  const [cancelRequested, setCancelRequested] = useState(false);
  const [failureMessage, setFailureMessage] = useState<string | null>(
    null,
  );
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

  useLayoutEffect(() => {
    setPhase("idle");
    setProgress(undefined);
    setCancellable(false);
    setCancelRequested(false);
    setFailureMessage(null);
    clearNotification();

    return () => {
      retireActiveAttempt();
      clearNotificationTimer();
    };
  }, [projectId]);

  function startExport() {
    if (disabled || currentAttemptId.current !== null) {
      return;
    }

    const attemptId = ++nextAttemptId.current;
    currentAttemptId.current = attemptId;
    beginInteraction();
    setPhase("starting");
    setProgress(undefined);
    setCancellable(false);
    clearNotification();
    setCancelRequested(false);
    setFailureMessage(null);

    let attempt: ExportAttempt;
    try {
      attempt = exportPort.startPreview((event) => {
        if (currentAttemptId.current !== attemptId) {
          return;
        }

        if (event.event === "started") {
          startedAttemptId.current = attemptId;
          setCancellable(event.cancellable);
          setPhase("running");
          return;
        }

        setCancellable(event.cancellable);
        setProgress(event);
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
        if (!finished) {
          return;
        }

        if (outcome.status === "cancelled") {
          if (finished.started) {
            setPhase("cancelled");
          } else {
            setPhase("idle");
            endInteraction();
          }
          return;
        }

        setPhase("idle");
        endInteraction();
        showNotification({
          kind: "success",
          message: "Exportação concluída",
        });
      },
      (error: unknown) => {
        finishAttemptWithFailure(attemptId, error);
      },
    );
  }

  function requestCancellation() {
    const current = activeAttempt.current;
    if (!current || current.cancelRequested) {
      return;
    }

    current.cancelRequested = true;
    setCancelRequested(true);
    void current.attempt.cancel().catch(() => undefined);
  }

  function dismissFeedback() {
    setPhase("idle");
    endInteraction();
  }

  function finishActiveAttempt(attemptId: number) {
    if (currentAttemptId.current !== attemptId) {
      return false;
    }

    currentAttemptId.current = null;
    activeAttempt.current = undefined;
    const started = startedAttemptId.current === attemptId;
    startedAttemptId.current = null;
    return { started };
  }

  function finishAttemptWithFailure(attemptId: number, error: unknown) {
    const finished = finishActiveAttempt(attemptId);
    if (!finished) {
      return;
    }

    const message = messageFromError(error);
    if (finished.started) {
      setFailureMessage(message);
      setPhase("failed");
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
    if (interactionActive.current) {
      return;
    }

    interactionActive.current = true;
    activeChangeListener.current = onActiveChange;
    onActiveChange?.(true);
  }

  function endInteraction() {
    if (!interactionActive.current) {
      return;
    }

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
      <button
        className="export-preview-trigger"
        type="button"
        disabled={disabled || phase !== "idle"}
        onClick={startExport}
      >
        Exportar prova
      </button>

      {phase === "running" && (
        <ExportModal title="Exportando">
          {progress ? (
            <>
              <p aria-live="polite">
                {progressStageLabel(progress.stage)}
              </p>
              {progress.units.kind === "measured" ? (
                <>
                  <progress
                    className="export-preview-progress"
                    aria-label="Progresso da exportação"
                    aria-valuemax={progress.units.totalUnits}
                    aria-valuenow={progress.units.completedUnits}
                    max={progress.units.totalUnits}
                    value={progress.units.completedUnits}
                  />
                  <p className="export-preview-count">
                    {progress.units.completedUnits} de{" "}
                    {progress.units.totalUnits}
                  </p>
                </>
              ) : (
                <progress
                  className="export-preview-progress"
                  aria-label="Progresso da exportação"
                />
              )}
            </>
          ) : (
            <>
              <p aria-live="polite">Iniciando a exportação</p>
              <progress
                className="export-preview-progress"
                aria-label="Progresso da exportação"
              />
            </>
          )}
          {cancellable && (
            <div className="export-preview-actions">
              <button
                type="button"
                disabled={cancelRequested}
                onClick={requestCancellation}
              >
                {cancelRequested
                  ? "Cancelando…"
                  : "Cancelar exportação"}
              </button>
            </div>
          )}
        </ExportModal>
      )}

      {(phase === "cancelled" || phase === "failed") && (
        <ExportModal
          title={
            phase === "cancelled"
              ? "Exportação cancelada"
              : "Exportação não concluída"
          }
        >
          <p>
            {phase === "cancelled"
              ? "A exportação foi cancelada."
              : failureMessage ??
                "Não foi possível exportar a prova."}
          </p>
          <div className="export-preview-actions">
            <button
              className="export-preview-action-primary"
              type="button"
              disabled={disabled}
              onClick={startExport}
            >
              Tentar novamente
            </button>
            <button type="button" onClick={dismissFeedback}>
              Fechar
            </button>
          </div>
        </ExportModal>
      )}

      {notification && (
        <div
          className={`export-preview-notification export-preview-notification-${notification.kind}`}
          role={notification.kind === "error" ? "alert" : "status"}
        >
          {notification.message}
        </div>
      )}
    </div>
  );
}

function messageFromError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
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

function ExportModal({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <ModalOverlay
      className="export-preview-backdrop"
      isKeyboardDismissDisabled
      isOpen
    >
      <Modal className="export-preview-modal">
        <Dialog aria-label={title} className="export-preview-dialog">
          <h2>{title}</h2>
          {children}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
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
