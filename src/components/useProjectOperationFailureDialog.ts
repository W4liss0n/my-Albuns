import { useEffect, useRef } from "react";

import type {
  ProjectDialogAction,
  ProjectDialogPort,
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
  const actionListenerRef = useRef<(action: ProjectDialogAction) => void>(
    () => undefined,
  );

  messageRef.current = message;
  onDismissRef.current = onDismiss;
  actionListenerRef.current = (action) => {
    if (action !== "dismissProjectOperationFailure") return;
    presentedMessageRef.current = null;
    onDismissRef.current();
    void projectDialogPort.dismiss().catch(() => undefined);
  };

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    void projectDialogPort
      .onAction((action) => actionListenerRef.current(action))
      .then((registeredUnsubscribe) => {
        if (active) unsubscribe = registeredUnsubscribe;
        else registeredUnsubscribe();
      })
      .catch(() => undefined);
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [projectDialogPort]);

  useEffect(() => {
    if (!message) {
      presentedMessageRef.current = null;
      return;
    }
    if (presentedMessageRef.current === message) return;
    presentedMessageRef.current = message;
    let active = true;
    void projectDialogPort
      .present({ kind: "projectOperationFailure", message })
      .catch(() => {
        if (!active || messageRef.current !== message) return;
        presentedMessageRef.current = null;
        onDismissRef.current();
      });
    return () => {
      active = false;
    };
  }, [message, projectDialogPort]);
}
