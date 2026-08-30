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
  ProjectCorePort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { ProjectDialogPort } from "./application/projectDialogPort";
import type { WorkspacePreferencesPort } from "./application/workspacePreferences";
import type { EditorProjection } from "./domain/project";
import { projectSaveAsStartupFailure } from "./application/projectSaveAsStartup";
import { LoggingProvider } from "./components/loggingContext";
import {
  CanvasGraphicsDiagnosticProbeProvider,
  type CanvasGraphicsDiagnosticProbe,
} from "./components/canvasGraphicsDiagnosticProbeContext";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import { useProjectCloseController } from "./components/useProjectCloseController";
import { useProjectMutationRunner } from "./components/useProjectMutationRunner";
import { useProjectOperationFailureDialog } from "./components/useProjectOperationFailureDialog";
import { useProjectGraphicsFailureDialog } from "./components/useProjectGraphicsFailureDialog";
import { BrandWordmark, InlineNotice } from "./ui";
import "./ui/theme.css";
import "./ui/ui.css";
import "./components/StartupSurface.css";
import "./App.css";

type AppProps = {
  exportPipelinePort: ExportPipelinePort;
  mediaPreviewPort: MediaPreviewPort;
  projectStartupPort: ProjectStartupPort;
  projectCorePort: ProjectCorePort;
  projectDialogPort: ProjectDialogPort;
  projectWindowPort: ProjectWindowPort;
  graphicsProbe: GraphicsProbe;
  canvasGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe;
  logger: Logger;
} & (
  | {
      workspacePreferencesPort: WorkspacePreferencesPort;
      workspacePreferencesMode?: never;
    }
  | {
      workspacePreferencesPort?: never;
      workspacePreferencesMode: "memory";
    }
);

interface MediaPreviewSubscription {
  projectId: string;
  port: MediaPreviewPort;
}

function App({
  exportPipelinePort,
  mediaPreviewPort,
  projectStartupPort,
  projectCorePort,
  projectDialogPort,
  projectWindowPort,
  graphicsProbe,
  canvasGraphicsDiagnosticProbe,
  logger,
  workspacePreferencesPort,
  workspacePreferencesMode,
}: AppProps) {
  const graphics = useMemo(() => graphicsProbe(), [graphicsProbe]);
  const [runtimeGraphicsDiagnostic, setRuntimeGraphicsDiagnostic] =
    useState<GraphicsDiagnostic | null>(null);
  const editorGraphics = runtimeGraphicsDiagnostic ?? graphics;
  const initialGraphicsFailure = !graphics.supported ? graphics : null;
  const runtimeGraphicsFailure =
    runtimeGraphicsDiagnostic && !runtimeGraphicsDiagnostic.supported
      ? runtimeGraphicsDiagnostic
      : null;
  const [projection, setProjection] = useState<EditorProjection | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
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
  const [initialGraphicsCloseError, setInitialGraphicsCloseError] = useState<
    string | null
  >(null);
  const [preferencesReadyProject, setPreferencesReadyProject] = useState("");
  const [mediaChangeSubscription, setMediaChangeSubscription] =
    useState<MediaPreviewSubscription | null>(null);
  const [cacheWarningSubscription, setCacheWarningSubscription] =
    useState<MediaPreviewSubscription | null>(null);
  const mediaDemandSequence = useRef({ projectId: "", revision: 0 });
  const uiReadyProject = useRef("");
  const loggerRef = useRef(logger);

  useEffect(() => {
    loggerRef.current = logger;
  }, [logger]);

  useProjectOperationFailureDialog({
    message:
      initialGraphicsCloseError ??
      saveAsStartupFailure ??
      cacheProcessorWarning?.message ??
      null,
    projectDialogPort,
    onDismiss: () => {
      setInitialGraphicsCloseError(null);
      const dismissedSaveAsFailure = saveAsStartupFailure !== null;
      setSaveAsStartupFailure(null);
      setCacheProcessorWarning(null);
      if (dismissedSaveAsFailure) {
        window.history.replaceState(
          window.history.state,
          "",
          `${window.location.pathname}${window.location.search}`,
        );
      }
    },
  });

  useEffect(() => {
    if (!graphics.supported) return;
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
  }, [graphics.supported, logger, projectCorePort]);

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
  const handlePreferencesReady = useCallback((readyProjectId: string) => {
    setPreferencesReadyProject(readyProjectId);
  }, []);
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
      preferencesReadyProject !== projectId ||
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
  }, [
    logger,
    mediaChangeListenerReady,
    preferencesReadyProject,
    projectId,
    projectStartupPort,
  ]);

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

  if (loadError) {
    return (
      <main className="startup-surface ui-chrome-selection-scope">
        <section className="startup-card" role="alert">
          <BrandWordmark compact />
          <p className="eyebrow">MyAlbuns</p>
          <h1>Não foi possível abrir o Projeto</h1>
          <InlineNotice tone="error">{loadError}</InlineNotice>
        </section>
      </main>
    );
  }

  if (initialGraphicsFailure) {
    return (
      <InitialProjectGraphicsFailureController
        diagnostic={initialGraphicsFailure}
        onCloseError={setInitialGraphicsCloseError}
        onProjectionChange={setProjection}
        projectDialogPort={projectDialogPort}
        projectWindowPort={projectWindowPort}
      />
    );
  }

  if (!projection) {
    return (
      <main className="startup-surface ui-chrome-selection-scope" aria-busy="true">
        <section className="startup-card">
          <BrandWordmark compact />
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
        <ProjectWorkspace
          projection={projection}
          exportPipelinePort={exportPipelinePort}
          projectDialogPort={projectDialogPort}
          projectWindowPort={projectWindowPort}
          runProjectMutation={runProjectMutation}
          projectCorePort={projectCorePort}
          mediaPreviews={mediaPreviews}
          onMediaDemandChange={updateMediaDemand}
          onRetryUnavailableMedia={retryUnavailableMedia}
          onProjectionChange={setProjection}
          onGraphicsUnavailable={setRuntimeGraphicsDiagnostic}
          graphicsFailure={runtimeGraphicsFailure}
          onPreferencesReady={handlePreferencesReady}
          workspacePreferences={
            workspacePreferencesPort
              ? { kind: "persistent", port: workspacePreferencesPort }
              : { kind: workspacePreferencesMode }
          }
        />
      </CanvasGraphicsDiagnosticProbeProvider>
    </LoggingProvider>
  );
}

const noPendingProjectMutations = async () => null;

function InitialProjectGraphicsFailureController({
  diagnostic,
  onCloseError,
  onProjectionChange,
  projectDialogPort,
  projectWindowPort,
}: {
  diagnostic: Extract<GraphicsDiagnostic, { supported: false }>;
  onCloseError(message: string): void;
  onProjectionChange(projection: EditorProjection): void;
  projectDialogPort: ProjectDialogPort;
  projectWindowPort: ProjectWindowPort;
}) {
  const projectClose = useProjectCloseController({
    onError: onCloseError,
    onProjectionChange,
    projectDialogPort,
    projectWindowPort,
    waitForPendingMutations: noPendingProjectMutations,
  });
  useProjectGraphicsFailureDialog({
    closeCancelRevision: projectClose.explicitCancelRevision,
    diagnostic,
    onCloseProject: projectClose.requestClose,
    projectDialogPort,
  });
  return null;
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

export default App;
