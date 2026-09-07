import { useEffect, useRef, useState } from "react";

import type { GraphicsDiagnostic } from "../application/graphics";
import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";

interface ProjectGraphicsFailureDialogOptions {
  closeCancelRevision: number;
  diagnostic: Extract<GraphicsDiagnostic, { supported: false }> | null;
  onCloseProject(): Promise<unknown> | unknown;
  projectDialogPort: ProjectDialogPort;
}

export function useProjectGraphicsFailureDialog({
  closeCancelRevision,
  diagnostic,
  onCloseProject,
  projectDialogPort,
}: ProjectGraphicsFailureDialogOptions) {
  const [rearmRevision, setRearmRevision] = useState(0);
  const closeCancelRevisionRef = useRef(closeCancelRevision);
  const closeTerminalWaitRef = useRef<number | null>(null);
  const mountedRef = useRef(true);
  const diagnosticRef = useRef(diagnostic);
  const onCloseProjectRef = useRef(onCloseProject);
  const presentationFailureRetryReasonRef = useRef<string | null>(null);
  const presentedReasonRef = useRef<string | null>(null);
  const sessionRef = useRef<ProjectDialogSession | null>(null);
  const actionListenerRef = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  diagnosticRef.current = diagnostic;
  closeCancelRevisionRef.current = closeCancelRevision;
  onCloseProjectRef.current = onCloseProject;

  const closeProjectAndRearmIfNeeded = async (rearmIfOpen: boolean) => {
    const closeOutcome = await Promise.resolve(
      onCloseProjectRef.current(),
    ).catch(() => null);
    if (isConfirmationRequiredOutcome(closeOutcome)) {
      if (mountedRef.current && diagnosticRef.current) {
        closeTerminalWaitRef.current = closeCancelRevisionRef.current;
      }
      return;
    }
    if (
      mountedRef.current &&
      rearmIfOpen &&
      diagnosticRef.current &&
      sessionRef.current === null &&
      !isClosedProjectOutcome(closeOutcome)
    ) {
      setRearmRevision((revision) => revision + 1);
    }
  };

  actionListenerRef.current = (action) => {
    if (action !== "closeProjectAfterGraphicsFailure") return;
    const session = sessionRef.current;
    if (!session) return;
    sessionRef.current = null;
    presentedReasonRef.current = null;
    void (async () => {
      await session.dismiss().catch(() => undefined);
      await closeProjectAndRearmIfNeeded(true);
    })();
  };

  useEffect(() => {
    const waitingSinceRevision = closeTerminalWaitRef.current;
    if (
      waitingSinceRevision === null ||
      closeCancelRevision <= waitingSinceRevision
    ) {
      return;
    }
    closeTerminalWaitRef.current = null;
    if (
      mountedRef.current &&
      diagnosticRef.current &&
      sessionRef.current === null
    ) {
      setRearmRevision((revision) => revision + 1);
    }
  }, [closeCancelRevision]);

  useEffect(() => {
    if (!diagnostic) {
      closeTerminalWaitRef.current = null;
      presentationFailureRetryReasonRef.current = null;
      presentedReasonRef.current = null;
      const session = sessionRef.current;
      sessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
      return;
    }
    if (
      sessionRef.current &&
      presentedReasonRef.current === diagnostic.reason
    ) {
      return;
    }

    const session =
      sessionRef.current ??
      projectDialogPort.acquire(
        (action) => actionListenerRef.current(action),
      );
    sessionRef.current = session;
    presentedReasonRef.current = diagnostic.reason;
    void session
      .present({ kind: "graphicsFailure", reason: diagnostic.reason })
      .then(() => {
        presentationFailureRetryReasonRef.current = null;
      })
      .catch(async () => {
        if (
          sessionRef.current !== session ||
          diagnosticRef.current?.reason !== diagnostic.reason
        ) {
          return;
        }
        sessionRef.current = null;
        presentedReasonRef.current = null;
        const rearmAfterFailure =
          presentationFailureRetryReasonRef.current !== diagnostic.reason;
        presentationFailureRetryReasonRef.current = diagnostic.reason;
        await session.dismiss().catch(() => undefined);
        await closeProjectAndRearmIfNeeded(rearmAfterFailure);
      });
  }, [diagnostic?.reason, projectDialogPort, rearmRevision]);

  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
      const session = sessionRef.current;
      sessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
    };
  }, []);
}

function isClosedProjectOutcome(value: unknown) {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    value.kind === "closed"
  );
}

function isConfirmationRequiredOutcome(value: unknown) {
  return (
    typeof value === "object" &&
    value !== null &&
    "kind" in value &&
    value.kind === "confirmationRequired"
  );
}
