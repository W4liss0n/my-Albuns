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
  const [cancelRequested, setCancelRequested] = useState(false);
  const [failureMessage, setFailureMessage] = useState<string | null>(
    null,
  );
  const [confirmationVisible, setConfirmationVisible] = useState(false);
  const nextAttemptId = useRef(0);
  const currentAttemptId = useRef<number | null>(null);
  const activeAttempt = useRef<{
    attempt: ExportAttempt;
    cancelRequested: boolean;
    id: number;
  } | undefined>(undefined);
  const activeChangeListener = useRef<
    ExportPreviewControlProps["onActiveChange"]
  >(undefined);
  const confirmationTimer = useRef<number | undefined>(undefined);

  useLayoutEffect(() => {
    setPhase("idle");
    setProgress(undefined);
    setCancelRequested(false);
    setFailureMessage(null);
    clearConfirmation();

    return () => {
      retireActiveAttempt();
      clearConfirmationTimer();
    };
  }, [projectId]);

  function startExport() {
    if (disabled || currentAttemptId.current !== null) {
      return;
    }

    const attemptId = ++nextAttemptId.current;
    currentAttemptId.current = attemptId;
    activeChangeListener.current = onActiveChange;
    onActiveChange?.(true);
    setPhase("starting");
    setProgress(undefined);
    clearConfirmation();
    setCancelRequested(false);
    setFailureMessage(null);

    let attempt: ExportAttempt;
    try {
      attempt = exportPort.startPreview((event) => {
        if (currentAttemptId.current !== attemptId) {
          return;
        }

        if (event.event === "started") {
          setProgress({
            event: "progress",
            stage: "preparing",
            completedUnits: 0,
            totalUnits: 1,
            cancellable: true,
          });
          setPhase("running");
          return;
        }

        setProgress(event);
      });
    } catch (error: unknown) {
      finishActiveAttempt(attemptId);
      setFailureMessage(messageFromError(error));
      setPhase("failed");
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
        if (!finishActiveAttempt(attemptId)) {
          return;
        }

        if (outcome.status === "cancelled") {
          setPhase("cancelled");
          return;
        }

        setPhase("idle");
        showConfirmation();
      },
      (error: unknown) => {
        if (finishActiveAttempt(attemptId)) {
          setFailureMessage(messageFromError(error));
          setPhase("failed");
        }
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
  }

  function finishActiveAttempt(attemptId: number) {
    if (currentAttemptId.current !== attemptId) {
      return false;
    }

    currentAttemptId.current = null;
    activeAttempt.current = undefined;
    const notify = activeChangeListener.current;
    activeChangeListener.current = undefined;
    notify?.(false);
    return true;
  }

  function retireActiveAttempt() {
    const attemptId = currentAttemptId.current;
    if (attemptId === null) {
      return;
    }

    const current = activeAttempt.current;
    if (current?.id === attemptId && !current.cancelRequested) {
      current.cancelRequested = true;
      void current.attempt.cancel().catch(() => undefined);
    }
    finishActiveAttempt(attemptId);
  }

  function clearConfirmationTimer() {
    if (confirmationTimer.current !== undefined) {
      window.clearTimeout(confirmationTimer.current);
      confirmationTimer.current = undefined;
    }
  }

  function clearConfirmation() {
    clearConfirmationTimer();
    setConfirmationVisible(false);
  }

  function showConfirmation() {
    clearConfirmationTimer();
    setConfirmationVisible(true);
    confirmationTimer.current = window.setTimeout(() => {
      confirmationTimer.current = undefined;
      setConfirmationVisible(false);
    }, 3_000);
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
              <progress
                className="export-preview-progress"
                aria-label="Progresso da exportação"
                aria-valuemax={progress.totalUnits}
                aria-valuenow={progress.completedUnits}
                max={progress.totalUnits}
                value={progress.completedUnits}
              />
              <p className="export-preview-count">
                {progress.completedUnits} de {progress.totalUnits}
              </p>
              {progress.cancellable &&
                progress.stage !== "publishing" && (
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
            </>
          ) : (
            <p>Iniciando…</p>
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

      {confirmationVisible && (
        <div className="export-preview-confirmation" role="status">
          Exportação concluída
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
