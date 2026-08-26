import { useEffect, useRef } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";

interface ProjectOperationFailureDialogOptions {
  message: string | null;
  projectDialogPort: ProjectDialogPort;
  onDismiss(): void;
}

export function useProjectOperationFailureDialog({
  message,
  projectDialogPort,
  onDismiss,
}: ProjectOperationFailureDialogOptions) {
  const messageRef = useRef(message);
  const onDismissRef = useRef(onDismiss);
  const presentedMessageRef = useRef<string | null>(null);
  const dialogSessionRef = useRef<ProjectDialogSession | null>(null);
  const actionListenerRef = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  messageRef.current = message;
  onDismissRef.current = onDismiss;
  actionListenerRef.current = (action) => {
    if (action !== "dismissProjectOperationFailure") return;
    presentedMessageRef.current = null;
    const session = dialogSessionRef.current;
    dialogSessionRef.current = null;
    onDismissRef.current();
    void session?.dismiss().catch(() => undefined);
  };

  useEffect(() => {
    if (!message) {
      presentedMessageRef.current = null;
      const session = dialogSessionRef.current;
      dialogSessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
      return;
    }
    if (presentedMessageRef.current === message) return;
    presentedMessageRef.current = message;
    let active = true;
    const session =
      dialogSessionRef.current ??
      projectDialogPort.acquire(
        (action) => actionListenerRef.current(action),
      );
    dialogSessionRef.current = session;
    void session
      .present({ kind: "projectOperationFailure", message })
      .catch(() => {
        if (
          !active ||
          messageRef.current !== message ||
          dialogSessionRef.current !== session
        ) {
          return;
        }
        presentedMessageRef.current = null;
        dialogSessionRef.current = null;
        void session.dismiss().catch(() => undefined);
        onDismissRef.current();
      });
    return () => {
      active = false;
    };
  }, [message, projectDialogPort]);

  useEffect(
    () => () => {
      const session = dialogSessionRef.current;
      dialogSessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
    },
    [],
  );
}
