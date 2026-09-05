import { useEffect, useRef } from "react";
import type {
  ProjectDialogPort,
  ProjectDialogSession,
} from "../application/projectDialogPort";
import type { ImageProcessingProgress } from "../application/projectPorts";

export function useImageProcessingProgressDialog(
  progress: ImageProcessingProgress | null,
  port: ProjectDialogPort,
) {
  const sessionRef = useRef<ProjectDialogSession | null>(null);

  useEffect(() => {
    if (!progress) {
      const session = sessionRef.current;
      sessionRef.current = null;
      void session?.dismiss().catch(() => undefined);
      return;
    }
    const session = sessionRef.current ?? port.acquire(() => undefined);
    sessionRef.current = session;
    void session
      .present({
        kind: "imageProcessingProgress",
        progress: progress.totalFiles > 0
          ? {
              kind: "determinate",
              completed: progress.completedFiles,
              total: progress.totalFiles,
              status: `${progress.completedFiles} de ${progress.totalFiles}`,
            }
          : { kind: "indeterminate", status: "Aguarde…" },
      })
      .catch(() => {
        if (sessionRef.current === session) sessionRef.current = null;
        void session.dismiss().catch(() => undefined);
      });
  }, [progress, port]);

  useEffect(() => () => {
    const session = sessionRef.current;
    sessionRef.current = null;
    void session?.dismiss().catch(() => undefined);
  }, [port]);
}
