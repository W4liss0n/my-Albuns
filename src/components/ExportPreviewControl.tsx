import {
  forwardRef,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
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
  ExportSheetSelection,
} from "../application/projectPorts";
import { ActionButton } from "../ui";
import { LayoutExportBlockedError } from "../application/projectPorts";
import "./ExportPreviewControl.css";
import { MediaExportBlockedError, type ExportMediaPort } from "../application/exportMedia";
import type { EditorProjection } from "../domain/project";
import { ExportConflictsError, type ExportSheetInfo, type NormalExportOptions } from "../application/normalExport";
import { StorageFullError, StorageRecoveryController, unavailableStorageRecovery } from "../application/storageRecovery";

interface ExportPreviewControlProps {
  sheets?: ExportSheetInfo[];
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
  start(scope?: "sheet" | "album"): void;
}

export const ExportPreviewControl = forwardRef<
  ExportPreviewControlHandle,
  ExportPreviewControlProps
>(function ExportPreviewControl(
  {
    sheets,
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
    "idle" | "configuring" | "starting" | "running" | "cancelled" | "completed" | "failed"
  >("idle");
  const nextAttemptId = useRef(0);
  const exportPercent = useRef(0);
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
  const resumeStorage = (id?: string) => {
    if (attemptedSelection.current) startSelectedExport({ ...attemptedSelection.current, recoveryId: id });
  };
  const cancelStorage = (id?: string) => {
    if (!id || !exportPipelinePort.discardRecovery) { dismissFeedback(); return; }
    void exportPipelinePort.discardRecovery(id).catch(() => undefined).finally(dismissFeedback);
  };
  const storageCallbacks = useRef({ presentDialog, resume: resumeStorage, cancel: cancelStorage });
  storageCallbacks.current = { presentDialog, resume: resumeStorage, cancel: cancelStorage };
  const storageController = useMemo(() => new StorageRecoveryController(
    exportPipelinePort.storageRecovery ?? unavailableStorageRecovery,
    state => storageCallbacks.current.presentDialog(state),
    id => storageCallbacks.current.resume(id), id => storageCallbacks.current.cancel(id),
  ), [exportPipelinePort.storageRecovery]);
  useLayoutEffect(() => () => storageController.dispose(), [storageController, projectId]);

  dialogActionListener.current = (action) => {
    if (typeof action !== "string") {
      if ("configureExport" in action && lastDialogState.current?.kind === "exportConfiguration" && !lastDialogState.current.busy) {
        startConfiguredExport(action.configureExport);
      } else if ("chooseExportDestination" in action) {
        void chooseDestination(action.chooseExportDestination);
      }
      return;
    }
    switch (action) {
      case "resumeStorage": case "clearStorageCache": case "cancelStorage":
        void storageController.act(action); break;
      case "confirmExportOverwrite":
      case "skipExportConflicts": {
        const selected = attemptedSelection.current;
        if (lastDialogState.current?.kind === "exportConflicts" && selected?.options) {
          startSelectedExport({ ...selected, options: { ...selected.options,
            conflictPolicy: action === "skipExportConflicts" ? "skip" : "replace",
          } });
        }
        break;
      }
      case "relinkExportMedia": void recoverMedia(true); break;
      case "retryExportMedia": void recoverMedia(false); break;
      case "cancelExport":
        requestCancellation();
        break;
      case "retryExport":
        retryExport();
        break;
      case "dismissExport":
      case "dismissImageProcessingProblems":
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

  function startExport(scope: "sheet" | "album" = "sheet") {
    if (!sheets) { startSelectedExport(selection); return; }
    if (disabled || !selection || phase !== "idle" || currentAttemptId.current !== null) return;
    beginInteraction();
    setPhase("configuring");
    const generation = ++recoveryGeneration.current;
    const options: NormalExportOptions = { scope: scope === "album" ? "album" : "range", sheetIds: scope === "album" ? sheets.map(sheet => sheet.sheetId) : [selection.sheetId], mode: "sheet", format: { kind: "jpeg", quality: 100 }, destination: "", conflictPolicy: "ask" as const };
    // Present once the initial fields are ready so opening never shrinks a busy form.
    void exportPipelinePort.defaultDestination().then(destination => {
      if (generation === recoveryGeneration.current) presentDialog({ kind: "exportConfiguration", sheets, options: { ...options, destination }, busy: false, message: "" });
    }, error => {
      if (generation === recoveryGeneration.current) presentDialog({ kind: "exportConfiguration", sheets, options, busy: false, message: messageFromError(error) });
    });
  }

  function startConfiguredExport(options: NormalExportOptions) {
    const sheet = sheets?.find(sheet => sheet.sheetId === options.sheetIds[0]);
    if (!selection || !sheet) return;
    startSelectedExport({ projectName: selection.projectName, sheetId: sheet.sheetId, sheetNumber: sheet.number, options });
  }

  async function chooseDestination(options: NormalExportOptions) {
    const current = lastDialogState.current;
    if (current?.kind !== "exportConfiguration" || current.busy || currentAttemptId.current !== null) return;
    const generation = ++recoveryGeneration.current;
    presentDialog({ ...current, options, busy: true, message: "" });
    try {
      const destination = await exportPipelinePort.chooseDestination();
      if (generation === recoveryGeneration.current) presentDialog({ ...current, options: { ...options, destination: destination ?? options.destination }, busy: false, message: "" });
    } catch (error) {
      if (generation === recoveryGeneration.current) presentDialog({ ...current, options, busy: false, message: messageFromError(error) });
    }
  }

  function startSelectedExport(selected: ExportSheetSelection | null) {
    if (disabled || !selected || currentAttemptId.current !== null) {
      return;
    }

    // A native recovery token is consumed by this call, never by a later fresh retry.
    attemptedSelection.current = { ...selected, recoveryId: undefined };
    if (!selected.recoveryId) exportPercent.current = 0;
    if (lastDialogState.current?.kind === "exportConfiguration") presentDialog({ ...lastDialogState.current, options: selected.options ?? lastDialogState.current.options, busy: true, message: "" });
    const attemptId = ++nextAttemptId.current;
    currentAttemptId.current = attemptId;
    beginInteraction();
    setPhase("starting");
    dialogPresentationFailed.current = false;

    let attempt: ExportAttempt;
    try {
      attempt = exportPipelinePort.startSheet(selected, (event) => {
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
              kind: "determinate", completed: exportPercent.current, total: 100,
              status: "Exportando",
            },
          });
          return;
        }

        const state = progressDialogState(event, exportPercent.current);
        exportPercent.current = state.progress.completed;
        presentDialog(state);
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

        if (outcome.status === "cancelled" || outcome.status === "skipped") {
          if (outcome.status === "cancelled" && finished.started) {
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
            lastDialogState.current = undefined;
            const session = dialogSession.current;
            dialogSession.current = null;
            void session?.dismiss().catch(() => undefined);
            endInteraction();
          }
          return;
        }

        setPhase("completed");
        presentDialog({
          kind: "exportSuccess",
          message: "A Exportação foi concluída com sucesso.",
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
      let problems;
      let message = "";
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
        if (result.problems.length === 0 && result.notes.length > 0) {
          presentDialog({ kind: "imageProcessingProblems", importedCount: null,
            operationProblem: null, problems: result.notes });
          return;
        }
        problems = result.problems;
        message = result.notes.map(note => `${note.fileName}: ${note.reason}`).join(" ");
      } else {
        problems = await exportMediaPort.inspect(selected);
        if (generation !== recoveryGeneration.current) return;
      }
      if (problems.length > 0) {
        presentDialog({ ...current, busy: false, problems, message });
        return;
      }
      if (dialogPresentationFailed.current) return;

      // Close Problems before the pipeline can open its destination picker.
      lastDialogState.current = undefined;
      const session = dialogSession.current;
      dialogSession.current = null;
      await session?.dismiss();
      if (generation !== recoveryGeneration.current) return;
      recoveryPending.current = false;
      startSelectedExport(selected);
    } catch (error) {
      if (generation === recoveryGeneration.current) {
        presentDialog(lastDialogState.current
          ? { ...current, busy: false, message: messageFromError(error) }
          : { kind: "exportFailure", cancelled: false, retryDisabled: false, message: messageFromError(error) });
      }
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
    startSelectedExport(attemptedSelection.current);
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
    if (lastDialogState.current?.kind === "exportConfiguration" && lastDialogState.current.busy) return;
    if (recoveryPending.current) return;
    if (lastDialogState.current?.kind === "exportMediaProblems" && lastDialogState.current.busy) return;
    if (
      phase !== "configuring" && phase !== "cancelled" &&
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
    if (error instanceof StorageFullError) {
      setPhase("failed");
      void storageController.open("export", message);
      return;
    }
    if (error instanceof ExportConflictsError) {
      setPhase("failed");
      presentDialog({ kind: "exportConflicts", files: error.files });
      return;
    }
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
        aria-label="Exportar"
        className="export-preview-trigger"
        disabled={disabled || !selection || phase !== "idle"}
        onClick={() => startExport("album")}
        variant="primary"
      >
        Exportar
      </ActionButton>

    </div>
  );
});

function progressDialogState(
  event: Extract<ExportProgressEvent, { event: "progress" }>,
  previous: number,
) {
  const fraction = event.units.kind === "measured" && event.units.totalUnits > 0
    ? Math.min(1, Math.max(0, event.units.completedUnits / event.units.totalUnits)) : 0;
  const ranges = { preparing: [0, 0], loading_sources: [0, 10], composing: [10, 65],
    encoding_output: [10, 65], verifying: [75, 10], publishing: [85, 14], completed: [100, 0] };
  const [start, span] = ranges[event.stage];
  return {
    cancelRequested: false,
    cancellable: event.cancellable,
    kind: "exportProgress" as const,
    progress: { completed: Math.max(previous, start + span * fraction), kind: "determinate" as const,
      status: "Exportando", total: 100 },
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
  return "Não foi possível concluir a Exportação.";
}
