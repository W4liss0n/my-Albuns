import { useEffect, useMemo, useRef } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";

import type { PhotoImportCompletion } from "../application/projectPorts";

interface ProjectOperationResultDialogOptions {
  importResult?: PhotoImportCompletion | null;
  message: string | null;
  projectDialogPort: ProjectDialogPort;
  onDismiss(kind: "projectOperationFailure" | "photoImportProblems"): void;
}

export function useProjectOperationResultDialog({
  message,
  importResult,
  projectDialogPort,
  onDismiss,
}: ProjectOperationResultDialogOptions) {
  const feedback = useMemo(() => {
    if (message) return { kind: "projectOperationFailure" as const, message };
    if (importResult?.problems.length) {
      return {
        kind: "photoImportProblems" as const,
        importedCount: importResult.importedCount,
        problems: importResult.problems,
      };
    }
    return null;
  }, [message, importResult]);
  const feedbackRef = useRef(feedback);
  const onDismissRef = useRef(onDismiss);
  const presentedFeedbackRef = useRef<typeof feedback>(null);
  const dialogSessionRef = useRef<ProjectDialogSession | null>(null);
  const actionListenerRef = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  feedbackRef.current = feedback;
  onDismissRef.current = onDismiss;
  actionListenerRef.current = (action) => {
    if (
      action !== "dismissProjectOperationFailure" &&
      action !== "dismissPhotoImportProblems"
    ) return;
    const kind = action === "dismissPhotoImportProblems" ? "photoImportProblems" : "projectOperationFailure";
    if (feedbackRef.current?.kind !== kind) return;
    presentedFeedbackRef.current = null;
    const session = dialogSessionRef.current;
    dialogSessionRef.current = null;
    onDismissRef.current(kind);
    void session?.dismiss().catch(() => undefined);
  };

  useEffect(() => {
    if (!feedback) {
      presentedFeedbackRef.current = null;
      const session = dialogSessionRef.current;
      dialogSessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
      return;
    }
    if (presentedFeedbackRef.current === feedback) return;
    presentedFeedbackRef.current = feedback;
    let active = true;
    const session =
      dialogSessionRef.current ??
      projectDialogPort.acquire(
        (action) => actionListenerRef.current(action),
      );
    dialogSessionRef.current = session;
    void session
      .present(feedback)
      .catch(() => {
        if (
          !active ||
          feedbackRef.current !== feedback ||
          dialogSessionRef.current !== session
        ) {
          return;
        }
        presentedFeedbackRef.current = null;
        dialogSessionRef.current = null;
        void session.dismiss().catch(() => undefined);
        onDismissRef.current(feedback.kind);
      });
    return () => {
      active = false;
    };
  }, [feedback, projectDialogPort]);

  useEffect(
    () => () => {
      const session = dialogSessionRef.current;
      dialogSessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
    },
    [],
  );
}
