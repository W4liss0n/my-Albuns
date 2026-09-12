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
import { LayoutExportBlockedError } from "../application/projectPorts";
import "./ExportPreviewControl.css";
import { MediaExportBlockedError, type ExportMediaPort } from "../application/exportMedia";
import type { EditorProjection } from "../domain/project";

interface ExportPreviewControlProps {
  exportMediaPort?: ExportMediaPort;
  onProjectionChange?(projection: EditorProjection): void;
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
    exportMediaPort,
    onProjectionChange,
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
  const recoveryGeneration = useRef(0);
  const recoveryPending = useRef(false);
  const attemptedSelection = useRef<ExportSheetSelection | null>(null);
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
      case "relinkExportMedia": void recoverMedia(true); break;
      case "retryExportMedia": void recoverMedia(false); break;
      case "continueMediaExport": {
        const current = lastDialogState.current;
        if (current?.kind === "exportMediaProblems" && !current.busy && current.problems.length === 0) startExport();
        break;
      }
      case "cancelExport":
        requestCancellation();
        break;
      case "retryExport":
        retryExport();
        break;
      case "dismissExport":
      case "openExportProject":
        dismissFeedback();
        break;
      default:
        break;
    }
  };

  useLayoutEffect(() => {
    recoveryGeneration.current++;
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

    attemptedSelection.current = { ...selection };
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

  async function recoverMedia(relink: boolean) {
    const current = lastDialogState.current;
    const selected = attemptedSelection.current;
    if (!exportMediaPort || !selected || current?.kind !== "exportMediaProblems" || current.busy) return;
    const generation = ++recoveryGeneration.current;
    recoveryPending.current = true;
    presentDialog({ ...current, busy: true, message: relink ? "Procurando e processando os Arquivos da pasta escolhida…" : "Verificando os Arquivos…" });
    try {
      if (relink) {
        const result = await exportMediaPort.relink(selected, progress => {
          if (generation !== recoveryGeneration.current) return;
          presentDialog({ kind: "imageProcessingProgress", progress: {
            kind: "determinate", completed: progress.completedFiles, total: progress.totalFiles,
            status: `${progress.completedFiles} de ${progress.totalFiles}`,
          } });
        });
        if (generation !== recoveryGeneration.current) return;
        onProjectionChange?.(result.projection);
        presentDialog({ ...current, busy: false, problems: result.problems,
          message: result.notes.map(note => `${note.fileName}: ${note.reason}`).join(" ") });
      } else {
        const problems = await exportMediaPort.inspect(selected);
        if (generation !== recoveryGeneration.current) return;
        presentDialog({ ...current, busy: false, problems, message: "" });
      }
    } catch (error) {
      if (generation === recoveryGeneration.current) presentDialog({ ...current, busy: false, message: messageFromError(error) });
    } finally {
      if (generation === recoveryGeneration.current) {
        recoveryPending.current = false;
        if (dialogPresentationFailed.current) {
          setPhase("idle");
          lastDialogState.current = undefined;
          endInteraction();
        }
      }
    }
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
    if (recoveryPending.current) return;
    if (lastDialogState.current?.kind === "exportMediaProblems" && lastDialogState.current.busy) return;
    if (
      phase !== "cancelled" &&
      phase !== "completed" &&
      phase !== "failed"
    ) {
      return;
    }
    setPhase("idle");
    recoveryGeneration.current++;
    lastDialogState.current = undefined;
    const session = dialogSession.current;
    dialogSession.current = null;
    void session?.dismiss().catch(() => undefined);
    endInteraction();
  }

  function presentDialog(state: ProjectDialogState) {
    if (recoveryPending.current && dialogPresentationFailed.current) return;
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
      } else if (!current && !recoveryPending.current) {
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
    if (error instanceof MediaExportBlockedError && attemptedSelection.current && exportMediaPort) {
      setPhase("failed");
      presentDialog({ kind: "exportMediaProblems", projectName: attemptedSelection.current.projectName, problems: error.problems, busy: false, message: "" });
      return;
    }
    if (error instanceof LayoutExportBlockedError && selection) {
      setPhase("failed");
      presentDialog({ kind: "exportProblems", projectName: selection.projectName, problems: error.problems });
      return;
    }
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
    recoveryGeneration.current++;
    recoveryPending.current = false;
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
