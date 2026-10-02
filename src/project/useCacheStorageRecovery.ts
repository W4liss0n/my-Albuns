import { useEffect, useRef } from "react";
import type { ProjectDialogPort, ProjectDialogSession } from "../application/projectDialogPort";
import type { ImageProcessingProblem, MediaPreviewPort } from "../application/projectPorts";
import { StorageRecoveryController } from "../application/storageRecovery";

export function useCacheStorageRecovery({ projectId, enabled, warning, port, dialogPort, onResumed }: {
  projectId: string; enabled: boolean; warning: boolean; port: MediaPreviewPort; dialogPort: ProjectDialogPort; onResumed(): void;
}) {
  const callbacks = useRef({ onResumed });
  callbacks.current = { onResumed };
  const controllerRef = useRef<StorageRecoveryController | null>(null);
  const running = useRef(false);
  const readyProject = useRef<string | null>(null);
  if (enabled) readyProject.current = projectId;
  const initialized = Boolean(projectId) && readyProject.current === projectId;
  useEffect(() => {
    if (!initialized || !port.storageRecovery || !port.resumeCacheImages) return;
    let active = true;
    let session: ProjectDialogSession | null = null;
    const ensureSession = () => session ??= dialogPort.acquire(action => {
      if (action === "dismissImageProcessingProblems") { dismiss(); callbacks.current.onResumed(); }
      if (action === "resumeStorage" || action === "clearStorageCache" || action === "cancelStorage") {
        void controller.act(action);
      }
    });
    const dismiss = () => {
      const previous = session;
      session = null;
      void previous?.dismiss().catch(() => undefined);
    };
    const message = "Libere espaço para continuar preparando as imagens.";
    const controller = new StorageRecoveryController(port.storageRecovery,
      state => { if (active) void ensureSession().present(state).catch(dismiss); },
      () => {
        running.current = true;
        const problems: ImageProcessingProblem[] = [];
        let operationProblem: string | null = null;
        void port.resumeCacheImages!(progress => {
          if (progress.problem) problems.push(progress.problem);
          if (progress.operationProblem) operationProblem = progress.operationProblem;
          if (active) void ensureSession().present({ kind: "imageProcessingProgress", progress: {
            kind: "determinate", completed: progress.completedFiles, total: progress.totalFiles,
            status: "Preparando imagens",
          } }).catch(dismiss);
        }).then(async completed => {
          if (!active) return;
          if (completed && (problems.length || operationProblem)) {
            await ensureSession().present({ kind: "imageProcessingProblems", importedCount: null, problems, operationProblem });
          } else if (completed) { dismiss(); callbacks.current.onResumed(); }
          else await controller.open("cache", message);
        }).catch(async () => { if (active) await controller.open("cache", message); })
          .finally(() => { running.current = false; });
      }, dismiss);
    controllerRef.current = controller;
    void controller.open("cache", message, true);
    return () => { active = false; controller.dispose(); controllerRef.current = null; dismiss(); };
  }, [initialized, projectId, port, dialogPort]);
  useEffect(() => {
    if (initialized && warning && !running.current) {
      void controllerRef.current?.open("cache", "Libere espaço para continuar preparando as imagens.");
    }
  }, [initialized, warning]);
}
