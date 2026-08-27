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
  ProjectDialogSession,
  ProjectDialogState,
} from "../application/projectDialogPort";
import type {
  ExportAttempt,
  ExportPipelinePort,
  ExportProgressEvent,
  ExportProgressStage,
  ExportSheetSelection,
} from "../application/projectPorts";
import { ActionButton } from "../ui";
import "./ExportPreviewControl.css";

interface ExportPreviewControlProps {
  dialogPort: ProjectDialogPort;
  disabled?: boolean;
  exportPipelinePort: ExportPipelinePort;
  onActiveChange?(active: boolean): void;
  projectId: string;
  selection: ExportSheetSelection | null;
}

export interface ExportPreviewControlHandle {
  start(): void;
}

export const ExportPreviewControl = forwardRef<
  ExportPreviewControlHandle,
  ExportPreviewControlProps
>(function ExportPreviewControl(
  {
    dialogPort,
    disabled = false,
    exportPipelinePort,
    onActiveChange,
    projectId,
    selection,
  },
  ref,
) {
  const [phase, setPhase] = useState<
    "idle" | "starting" | "running" | "cancelled" | "completed" | "failed"
  >("idle");
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
  const lastDialogState = useRef<ProjectDialogState | undefined>(undefined);
  const dialogPresentationFailed = useRef(false);
  const dialogSession = useRef<ProjectDialogSession | null>(null);
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
    setPhase("idle");
    lastDialogState.current = undefined;
    dialogPresentationFailed.current = false;
    const previousSession = dialogSession.current;
    dialogSession.current = null;
    void previousSession?.dismiss().catch(() => undefined);

    return () => {
      retireActiveAttempt();
      lastDialogState.current = undefined;
      const session = dialogSession.current;
      dialogSession.current = null;
      void session?.dismiss().catch(() => undefined);
    };
  }, [dialogPort, projectId]);

  function startExport() {
    if (disabled || !selection || currentAttemptId.current !== null) {
      return;
    }

    const attemptId = ++nextAttemptId.current;
    currentAttemptId.current = attemptId;
    beginInteraction();
    setPhase("starting");
    dialogPresentationFailed.current = false;

    let attempt: ExportAttempt;
    try {
      attempt = exportPipelinePort.startSheet(selection, (event) => {
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

        setPhase("completed");
        presentDialog({
          kind: "exportSuccess",
          message: "A prova foi exportada com sucesso.",
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
    if (
      phase !== "cancelled" &&
      phase !== "completed" &&
      phase !== "failed"
    ) {
      return;
    }
    setPhase("idle");
    lastDialogState.current = undefined;
    const session = dialogSession.current;
    dialogSession.current = null;
    void session?.dismiss().catch(() => undefined);
    endInteraction();
  }

  function presentDialog(state: ProjectDialogState) {
    lastDialogState.current = state;
    const session =
      dialogSession.current ??
      dialogPort.acquire(
        (action) => dialogActionListener.current(action),
      );
    dialogSession.current = session;
    void session.present(state).catch(() => {
      if (dialogSession.current !== session) return;
      if (dialogPresentationFailed.current) return;
      dialogPresentationFailed.current = true;
      dialogSession.current = null;
      void session.dismiss().catch(() => undefined);
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

    setPhase("failed");
    presentDialog({
      cancelled: false,
      kind: "exportFailure",
      message,
      retryDisabled: false,
    });
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

  return (
    <div className="export-preview-control">
      <ActionButton
        aria-label="Exportar Lâmina"
        className="export-preview-trigger"
        disabled={disabled || !selection || phase !== "idle"}
        onClick={startExport}
        variant="primary"
      >
        Exportar
      </ActionButton>

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
