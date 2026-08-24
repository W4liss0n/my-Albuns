import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import type {
  GraphicsDiagnostic,
  GraphicsProbe,
} from "./application/graphics";
import {
  createLogInstanceId,
  logReasonFromError,
  type Logger,
} from "./application/logging";
import type {
  ExportPipelinePort,
  CacheProcessorWarning,
  MediaPreview,
  MediaPreviewDemand,
  MediaPreviewPort,
  ProjectStartupPort,
  ProjectRecoveryDecision,
  ProjectCorePort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { EditorProjection } from "./domain/project";
import { projectSaveAsStartupFailure } from "./application/projectSaveAsStartup";
import { LoggingProvider } from "./components/loggingContext";
import {
  CanvasGraphicsDiagnosticProbeProvider,
  type CanvasGraphicsDiagnosticProbe,
} from "./components/canvasGraphicsDiagnosticProbeContext";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import { useProjectMutationRunner } from "./components/useProjectMutationRunner";
import "./App.css";

interface AppProps {
  exportPipelinePort: ExportPipelinePort;
  mediaPreviewPort: MediaPreviewPort;
  projectStartupPort: ProjectStartupPort;
  projectCorePort: ProjectCorePort;
  projectWindowPort: ProjectWindowPort;
  graphicsProbe: GraphicsProbe;
  canvasGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe;
  logger: Logger;
}

interface MediaPreviewSubscription {
  projectId: string;
  port: MediaPreviewPort;
}

type RecoveryStartupState =
  | "checking"
  | "none"
  | "available"
  | "confirmDiscard"
  | "resolving"
  | "resolved"
  | "deferred";

function App({
  exportPipelinePort,
  mediaPreviewPort,
  projectStartupPort,
  projectCorePort,
  projectWindowPort,
  graphicsProbe,
  canvasGraphicsDiagnosticProbe,
  logger,
}: AppProps) {
  const graphics = useMemo(() => graphicsProbe(), [graphicsProbe]);
  const [runtimeGraphicsDiagnostic, setRuntimeGraphicsDiagnostic] =
    useState<GraphicsDiagnostic | null>(null);
  const editorGraphics = runtimeGraphicsDiagnostic ?? graphics;
  const [projection, setProjection] = useState<EditorProjection | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [recoveryStartup, setRecoveryStartup] =
    useState<RecoveryStartupState>("checking");
  const [recoveryError, setRecoveryError] = useState<string | null>(null);
  const [mediaPreviews, setMediaPreviews] = useState<
    Readonly<Record<string, MediaPreview>>
  >({});
  const [mediaDemand, setMediaDemand] = useState<MediaPreviewDemand>({
    visibleMediaIds: [],
    preloadMediaIds: [],
  });
  const [mediaRefreshRevision, setMediaRefreshRevision] = useState(0);
  const [cacheProcessorWarning, setCacheProcessorWarning] =
    useState<CacheProcessorWarning | null>(null);
  const [saveAsStartupFailure, setSaveAsStartupFailure] = useState(() =>
    projectSaveAsStartupFailure(window.location.hash),
  );
  const [mediaChangeSubscription, setMediaChangeSubscription] =
    useState<MediaPreviewSubscription | null>(null);
  const [cacheWarningSubscription, setCacheWarningSubscription] =
    useState<MediaPreviewSubscription | null>(null);
  const mediaDemandSequence = useRef({ projectId: "", revision: 0 });
  const uiReadyProject = useRef("");
  const recoveryUiReady = useRef(false);
  const loggerRef = useRef(logger);

  useEffect(() => {
    loggerRef.current = logger;
  }, [logger]);

  useEffect(() => {
    if (!graphics.supported) return;
    let active = true;
    projectStartupPort.recoveryStatus().then(
      (status) => {
        if (!active) return;
        setRecoveryStartup(status.kind === "available" ? "available" : "none");
      },
      (error: unknown) => {
        if (!active) return;
        setLoadError(
          error instanceof Error
            ? error.message
            : "Não foi possível verificar a Recuperação do Projeto.",
        );
      },
    );
    return () => {
      active = false;
    };
  }, [graphics.supported, projectStartupPort]);

  useEffect(() => {
    if (!graphics.supported || recoveryStartup !== "none") return;
    let active = true;
    const operationId = createLogInstanceId("project-load");
    logger.write({
      level: "info",
      component: "application",
      event: "project_load_started",
      operationId,
    });
    projectCorePort
      .load(operationId)
      .then((value) => {
        if (active) {
          logger.write({
            level: "info",
            component: "application",
            event: "project_load_completed",
            operationId,
            projectId: value.state.projectId,
            sheetCount: value.composition.sheets.length,
          });
          setMediaDemand({ visibleMediaIds: [], preloadMediaIds: [] });
          setProjection(value);
        }
      })
      .catch((error: unknown) => {
        if (active) {
          logger.write({
            level: "error",
            component: "application",
            event: "project_load_failed",
            operationId,
            reason: "bridge_error",
          });
          setLoadError(
            error instanceof Error
              ? error.message
              : "Não foi possível iniciar a Sessão do Projeto.",
          );
        }
      });
    return () => {
      active = false;
    };
  }, [graphics.supported, logger, projectCorePort, recoveryStartup]);

  useEffect(() => {
    if (
      recoveryUiReady.current ||
      !["available", "confirmDiscard", "resolving"].includes(recoveryStartup)
    ) {
      return;
    }
    recoveryUiReady.current = true;
    projectStartupPort.confirmUiReady().catch((error: unknown) => {
      recoveryUiReady.current = false;
      logger.write({
        level: "error",
        component: "application",
        event: "project_recovery_ui_ready_failed",
        reason: logReasonFromError(error),
      });
      setLoadError("Não foi possível confirmar a interface de Recuperação.");
    });
  }, [logger, projectStartupPort, recoveryStartup]);

  const resolveRecovery = useCallback(
    async (decision: ProjectRecoveryDecision) => {
      setRecoveryError(null);
      setRecoveryStartup("resolving");
      try {
        const resolution = await projectStartupPort.resolveRecovery(decision);
        if (resolution.kind === "deferred") {
          setRecoveryStartup("deferred");
          return;
        }
        setMediaDemand({ visibleMediaIds: [], preloadMediaIds: [] });
        setProjection(resolution.projection);
        setRecoveryStartup("resolved");
      } catch (error: unknown) {
        setRecoveryError(
          error instanceof Error
            ? error.message
            : "Não foi possível concluir a escolha de Recuperação.",
        );
        setRecoveryStartup(
          decision === "discardCheckpointAndOpenLastSaved"
            ? "confirmDiscard"
            : "available",
        );
      }
    },
    [projectStartupPort],
  );

  useEffect(() => {
    logger.write({
      level: graphics.supported ? "info" : "warn",
      component: "graphics",
      event: graphics.supported
        ? "graphics_probe_succeeded"
        : "graphics_probe_failed",
      reason: graphics.supported
        ? undefined
        : graphics.code,
    });
  }, [graphics, logger]);

  const projectId = projection?.state.projectId ?? "";
  const retryUnavailableMedia = useCallback(
    async (mediaId: string) => {
      const operationId = createLogInstanceId("media-retry");
      logger.write({
        level: "info",
        component: "media-preview",
        event: "media_retry_started",
        operationId,
        projectId,
      });
      try {
        const preview = await mediaPreviewPort.retryUnavailableMedia(mediaId);
        if (preview.state !== "ready") {
          setMediaPreviews((current) => ({
            ...current,
            [mediaId]: preview,
          }));
        }
        logger.write({
          level: "info",
          component: "media-preview",
          event: "media_retry_completed",
          operationId,
          projectId,
        });
      } catch (error: unknown) {
        logger.write({
          level: "warn",
          component: "media-preview",
          event: "media_retry_failed",
          operationId,
          projectId,
          reason: logReasonFromError(error),
        });
      }
    },
    [logger, mediaPreviewPort, projectId],
  );
  const updateMediaDemand = useCallback((next: MediaPreviewDemand) => {
    setMediaDemand((current) =>
      sameMediaDemand(current, next) ? current : next,
    );
  }, []);
  const runProjectMutation = useProjectMutationRunner(
    projectId,
    projectCorePort,
  );
  useEffect(() => {
    if (!projectId) return;
    let active = true;
    let latestProjectionRefresh = 0;
    let unlisten: (() => void) | undefined;
    void mediaPreviewPort
      .onMediaChanged(() => {
        if (!active) return;
        setMediaRefreshRevision((revision) => revision + 1);
        const refresh = ++latestProjectionRefresh;
        const operationId = createLogInstanceId("media-refresh");
        void projectCorePort.load(operationId).then(
          (refreshed) => {
            if (
              !active ||
              refresh !== latestProjectionRefresh ||
              refreshed.state.projectId !== projectId
            ) {
              return;
            }
            setProjection((current) =>
              current?.state.projectId === projectId &&
              current.state.revision > refreshed.state.revision
                ? current
                : refreshed,
            );
          },
          (error: unknown) => {
            if (!active || refresh !== latestProjectionRefresh) return;
            logger.write({
              level: "warn",
              component: "media-preview",
              event: "media_projection_refresh_failed",
              operationId,
              projectId,
              reason: logReasonFromError(error),
            });
          },
        );
      })
      .then((dispose) => {
        if (active) {
          unlisten = dispose;
          setMediaChangeSubscription({
            projectId,
            port: mediaPreviewPort,
          });
        } else {
          dispose();
        }
      })
      .catch((error: unknown) => {
        if (!active) return;
        logger.write({
          level: "warn",
          component: "media-preview",
          event: "media_monitor_subscription_failed",
          projectId,
          reason: logReasonFromError(error),
        });
      });
    return () => {
      active = false;
      unlisten?.();
      setMediaChangeSubscription((current) =>
        current?.projectId === projectId && current.port === mediaPreviewPort
          ? null
          : current,
      );
    };
  }, [logger, mediaPreviewPort, projectCorePort, projectId]);

  useEffect(() => {
    setCacheProcessorWarning(null);
    if (!projectId) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void mediaPreviewPort
      .onCacheProcessorWarning((warning) => {
        if (active) setCacheProcessorWarning(warning);
      })
      .then((dispose) => {
        if (active) {
          unlisten = dispose;
          setCacheWarningSubscription({
            projectId,
            port: mediaPreviewPort,
          });
        } else {
          dispose();
        }
      })
      .catch((error: unknown) => {
        if (!active) return;
        loggerRef.current.write({
          level: "warn",
          component: "media-preview",
          event: "cache_processor_warning_subscription_failed",
          projectId,
          reason: logReasonFromError(error),
        });
      });
    return () => {
      active = false;
      unlisten?.();
      setCacheWarningSubscription((current) =>
        current?.projectId === projectId && current.port === mediaPreviewPort
          ? null
          : current,
      );
    };
  }, [mediaPreviewPort, projectId]);

  useEffect(() => {
    setMediaPreviews({});
  }, [projectId]);

  const cacheWarningListenerReady =
    cacheWarningSubscription?.projectId === projectId &&
    cacheWarningSubscription.port === mediaPreviewPort;
  const mediaChangeListenerReady =
    mediaChangeSubscription?.projectId === projectId &&
    mediaChangeSubscription.port === mediaPreviewPort;

  useEffect(() => {
    if (
      !projectId ||
      !mediaChangeListenerReady ||
      uiReadyProject.current === projectId
    ) {
      return;
    }
    uiReadyProject.current = projectId;
    projectStartupPort.confirmUiReady().catch((error: unknown) => {
      if (uiReadyProject.current === projectId) {
        uiReadyProject.current = "";
      }
      logger.write({
        level: "error",
        component: "application",
        event: "project_ui_ready_failed",
        projectId,
        reason: logReasonFromError(error),
      });
      setLoadError("Não foi possível confirmar a inicialização da interface do Projeto.");
    });
  }, [logger, mediaChangeListenerReady, projectId, projectStartupPort]);

  useEffect(() => {
    if (
      !projectId ||
      !cacheWarningListenerReady ||
      !mediaChangeListenerReady
    ) {
      return;
    }
    if (mediaDemandSequence.current.projectId !== projectId) {
      mediaDemandSequence.current = { projectId, revision: 0 };
    }
    const effectiveDemand = editorGraphics.supported
      ? mediaDemand
      : { visibleMediaIds: [], preloadMediaIds: [] };
    const demandIsEmpty =
      effectiveDemand.visibleMediaIds.length === 0 &&
      effectiveDemand.preloadMediaIds.length === 0;
    if (demandIsEmpty && mediaDemandSequence.current.revision === 0) {
      return;
    }
    const demand = {
      ...effectiveDemand,
      revision: ++mediaDemandSequence.current.revision,
    };

    let active = true;
    const operationId = createLogInstanceId("media-preview");
    logger.write({
      level: "info",
      component: "media-preview",
      event: "media_preview_started",
      operationId,
      projectId,
    });
    mediaPreviewPort
      .prepareMediaPreviews(demand)
      .then((previews) => {
        if (!active) return;
        setMediaPreviews(
          Object.fromEntries(
            (previews ?? []).map((preview) => [preview.mediaId, preview]),
          ),
        );
        logger.write({
          level: "info",
          component: "media-preview",
          event: "media_preview_completed",
          operationId,
          projectId,
        });
      })
      .catch((error: unknown) => {
        if (!active) return;
        logger.write({
          level: "warn",
          component: "media-preview",
          event: "media_preview_failed",
          operationId,
          projectId,
          reason: logReasonFromError(error),
        });
      });
    return () => {
      active = false;
    };
  }, [
    cacheWarningListenerReady,
    editorGraphics.supported,
    logger,
    mediaDemand,
    mediaChangeListenerReady,
    mediaRefreshRevision,
    mediaPreviewPort,
    projectId,
  ]);

  if (!editorGraphics.supported) {
    return <ProjectGraphicsFailure diagnostic={editorGraphics} />;
  }

  if (loadError) {
    return (
      <main className="startup-surface">
        <section className="startup-card" role="alert">
          <p className="eyebrow">MyAlbuns</p>
          <h1>Não foi possível abrir o Projeto</h1>
          <p>{loadError}</p>
        </section>
      </main>
    );
  }

  if (recoveryStartup === "deferred") {
    return (
      <main className="startup-surface" aria-busy="true">
        <section className="startup-card">
          <span className="loading-mark" aria-hidden="true" />
          <p>Fechando o Projeto…</p>
        </section>
      </main>
    );
  }

  if (recoveryStartup === "confirmDiscard") {
    return (
      <main className="startup-surface">
        <section className="startup-card">
          <p className="eyebrow">Recuperação de sessão</p>
          <h1>Descartar o trabalho recuperável?</h1>
          <p>
            A última versão salva será aberta e o trabalho recuperável será
            removido definitivamente.
          </p>
          {recoveryError && <p role="alert">{recoveryError}</p>}
          <div className="recovery-actions">
            <button
              className="recovery-primary"
              type="button"
              onClick={() =>
                void resolveRecovery("discardCheckpointAndOpenLastSaved")
              }
            >
              Descartar recuperação e abrir
            </button>
            <button
              type="button"
              onClick={() => {
                setRecoveryError(null);
                setRecoveryStartup("available");
              }}
            >
              Voltar
            </button>
          </div>
        </section>
      </main>
    );
  }

  if (recoveryStartup === "available" || recoveryStartup === "resolving") {
    const busy = recoveryStartup === "resolving";
    return (
      <main className="startup-surface">
        <section className="startup-card">
          <p className="eyebrow">Recuperação de sessão</p>
          <h1>Recuperar trabalho não salvo?</h1>
          <p>
            O MyAlbuns encontrou trabalho concluído depois da última versão
            salva deste Projeto.
          </p>
          {recoveryError && <p role="alert">{recoveryError}</p>}
          <div className="recovery-actions">
            <button
              className="recovery-primary"
              disabled={busy}
              type="button"
              onClick={() => void resolveRecovery("reopenAndRecover")}
            >
              Reabrir e recuperar
            </button>
            <button
              disabled={busy}
              type="button"
              onClick={() => setRecoveryStartup("confirmDiscard")}
            >
              Abrir última versão salva
            </button>
            <button
              disabled={busy}
              type="button"
              onClick={() => void resolveRecovery("nowNot")}
            >
              Agora não
            </button>
          </div>
          {busy && <p aria-live="polite">Concluindo…</p>}
        </section>
      </main>
    );
  }

  if (!projection) {
    return (
      <main className="startup-surface" aria-busy="true">
        <section className="startup-card">
          <span className="loading-mark" aria-hidden="true" />
          <p>Preparando o editor…</p>
        </section>
      </main>
    );
  }

  return (
    <LoggingProvider logger={logger}>
      <CanvasGraphicsDiagnosticProbeProvider
        probe={canvasGraphicsDiagnosticProbe}
      >
        {cacheProcessorWarning && (
          <aside
            className="cache-runtime-warning"
            role="status"
            aria-label="Cache suspenso"
          >
            <strong>Cache suspenso</strong>
            <span>{cacheProcessorWarning.message}</span>
            <small>A edição e o Salvamento continuam disponíveis.</small>
          </aside>
        )}
        {saveAsStartupFailure && (
          <aside
            className="operation-toast error save-as-startup-terminal"
            role="alert"
          >
            <div>
              <strong>Não foi possível concluir Salvar como</strong>
              <span>{saveAsStartupFailure}</span>
            </div>
            <button
              type="button"
              aria-label="Fechar mensagem"
              onClick={() => {
                setSaveAsStartupFailure(null);
                window.history.replaceState(
                  window.history.state,
                  "",
                  `${window.location.pathname}${window.location.search}`,
                );
              }}
            >
              ×
            </button>
          </aside>
        )}
        <ProjectWorkspace
          projection={projection}
          exportPipelinePort={exportPipelinePort}
          projectWindowPort={projectWindowPort}
          runProjectMutation={runProjectMutation}
          projectCorePort={projectCorePort}
          mediaPreviews={mediaPreviews}
          onMediaDemandChange={updateMediaDemand}
          onRetryUnavailableMedia={retryUnavailableMedia}
          onProjectionChange={setProjection}
          onGraphicsUnavailable={setRuntimeGraphicsDiagnostic}
        />
      </CanvasGraphicsDiagnosticProbeProvider>
    </LoggingProvider>
  );
}

function sameMediaDemand(
  left: MediaPreviewDemand,
  right: MediaPreviewDemand,
) {
  return (
    sameStrings(left.visibleMediaIds, right.visibleMediaIds) &&
    sameStrings(left.preloadMediaIds, right.preloadMediaIds)
  );
}

function sameStrings(left: readonly string[], right: readonly string[]) {
  return (
    left.length === right.length &&
    left.every((value, index) => value === right[index])
  );
}

function ProjectGraphicsFailure({
  diagnostic,
}: {
  diagnostic: Extract<GraphicsDiagnostic, { supported: false }>;
}) {
  return (
    <main className="startup-surface">
      <section className="startup-card" role="alert">
        <p className="eyebrow">Editor indisponível</p>
        <h1>O Canvas não pôde ser iniciado</h1>
        <p>{diagnostic.reason}</p>
        <p className="support-note">
          Feche esta Janela do Projeto e use o Diagnóstico gráfico da
          Boas-vindas antes de tentar novamente.
        </p>
      </section>
    </main>
  );
}

export default App;
