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
  MediaDropPort,
  MediaFileCatalog,
  ImageProcessingProgress,
  ImageProcessingProblem,
  MediaPreviewDemand,
  MediaPreviewPort,
  ProjectStartupPort,
  ProjectCorePort,
  MediaImportCompletion,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { ProjectDialogPort } from "./application/projectDialogPort";
import type { WorkspacePreferencesPort } from "./application/workspacePreferences";
import type { EditorProjection } from "./domain/project";
import { projectSaveAsStartupFailure } from "./application/projectSaveAsStartup";
import { decodeMediaPreview } from "./application/mediaPreviews";
import { LoggingProvider } from "./components/loggingContext";
import {
  CanvasGraphicsDiagnosticProbeProvider,
  type CanvasGraphicsDiagnosticProbe,
} from "./components/canvasGraphicsDiagnosticProbeContext";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import { useProjectCloseController } from "./components/useProjectCloseController";
import { useProjectMutationRunner } from "./components/useProjectMutationRunner";
import { useProjectOperationResultDialog } from "./components/useProjectOperationResultDialog";
import { useProjectGraphicsFailureDialog } from "./components/useProjectGraphicsFailureDialog";
import { BrandWordmark, InlineNotice } from "./ui";
import "./ui/theme.css";
import "./ui/ui.css";
import "./components/StartupSurface.css";
import "./App.css";

type AppProps = {
  photoshopPort?: import("./application/photoshop").PhotoshopPort;
  exportPipelinePort: ExportPipelinePort;
  mediaPreviewPort: MediaPreviewPort;
  mediaDropPort?: MediaDropPort;
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

interface ImportPresentation {
  projectId: string;
  revision: number;
  ready: boolean;
  cancel(): void;
}

function App({
  photoshopPort,
  exportPipelinePort,
  mediaPreviewPort,
  mediaDropPort,
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
  const [initialImageProblems, setInitialImageProblems] = useState<readonly ImageProcessingProblem[]>([]);
  const [mediaPreviews, setMediaPreviews] = useState<
    Readonly<Record<string, MediaPreview>>
  >({});
  const [mediaDemand, setMediaDemand] = useState<MediaPreviewDemand>({
    visibleMediaIds: [],
    preloadMediaIds: [],
  });
  const [mediaRefreshRevision, setMediaRefreshRevision] = useState(0);
  const [mediaFileCatalog, setMediaFileCatalog] = useState<MediaFileCatalog | null>(null);
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
  const importPresentation = useRef<ImportPresentation | null>(null);
  const preparedPresentation = useRef<{
    projectId: string;
    demand: MediaPreviewDemand;
    refreshRevision: number;
    previews: Readonly<Record<string, MediaPreview>>;
  } | null>(null);
  const projectionRef = useRef(projection);
  projectionRef.current = projection;
  const uiReadyProject = useRef("");
  const loggerRef = useRef(logger);

  useEffect(() => {
    loggerRef.current = logger;
  }, [logger]);

  useProjectOperationResultDialog({
    processingProblems: initialImageProblems,
    message:
      initialGraphicsCloseError ??
      saveAsStartupFailure ??
      cacheProcessorWarning?.message ??
      null,
    projectDialogPort,
    onDismiss: (kind) => {
      if (kind === "imageProcessingProblems") {
        setInitialImageProblems([]);
        return;
      }
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
  const mediaFiles = useMemo(() => Object.fromEntries(
    mediaFileCatalog?.projectId === projectId ? mediaFileCatalog.files.map((file) => [file.mediaId, file]) : [],
  ), [mediaFileCatalog, projectId]);
  useEffect(() => {
    if (!projectId) return;
    let active = true;
    void mediaPreviewPort.readMediaFiles().then((catalog) => {
      if (active && catalog.projectId === projectId) setMediaFileCatalog(catalog);
    }).catch((error: unknown) => {
      if (active) loggerRef.current.write({ level: "warn", component: "media-preview",
        event: "media_file_information_failed", projectId, reason: logReasonFromError(error) });
    });
    return () => { active = false; };
  }, [projectId, projection?.state.revision, mediaRefreshRevision, mediaPreviewPort]);
  const handlePreferencesReady = useCallback((readyProjectId: string) => {
    setPreferencesReadyProject(readyProjectId);
  }, []);
  const retryUnavailableMedia = useCallback(
    async (mediaId: string, onProgress: (progress: ImageProcessingProgress) => void) => {
      const operationId = createLogInstanceId("media-retry");
      logger.write({
        level: "info",
        component: "media-preview",
        event: "media_retry_started",
        operationId,
        projectId,
      });
      try {
        const preview = await mediaPreviewPort.retryUnavailableMedia(mediaId, onProgress);
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
    const batch = importPresentation.current;
    const currentProjection = projectionRef.current;
    // Keep the prepared URLs resident until the committed panel reports demand.
    if (batch?.ready && currentProjection?.state.projectId === batch.projectId &&
      currentProjection.state.revision >= batch.revision) {
      importPresentation.current = null;
      const prepared = preparedPresentation.current;
      if (prepared?.projectId === batch.projectId) setMediaPreviews(prepared.previews);
    }
    setMediaDemand((current) =>
      sameMediaDemand(current, next) ? current : next,
    );
  }, []);

  useEffect(() => () => {
    importPresentation.current?.cancel();
    importPresentation.current = null;
    preparedPresentation.current = null;
  }, [projectId, mediaPreviewPort]);

  const prepareMediaPresentation = useCallback(async (
    completion: MediaImportCompletion,
    demand: MediaPreviewDemand,
  ): Promise<readonly ImageProcessingProblem[]> => {
    const imported = completion.projection;
    if (imported.state.projectId !== projectId || projectionRef.current?.state.projectId !== projectId) {
      throw new Error("O Projeto mudou antes da entrega das miniaturas.");
    }
    let cancel!: () => void;
    const cancelled = new Promise<never>((_resolve, reject) => {
      cancel = () => reject(new Error("A preparação das miniaturas foi interrompida pela troca de Projeto."));
    });
    const batch: ImportPresentation = {
      projectId, revision: imported.state.revision, ready: false, cancel,
    };
    importPresentation.current = batch;
    if (mediaDemandSequence.current.projectId !== projectId) {
      mediaDemandSequence.current = { projectId, revision: 0 };
    }
    const request = { ...demand, revision: ++mediaDemandSequence.current.revision };
    const problems: ImageProcessingProblem[] = [];
    const importedMediaIds = new Set(completion.mediaIds);
    const mediaById = new Map(imported.state.album.media.map((media) => [media.id, media]));
    const reportUnavailablePreview = (mediaId: string) => {
      const media = mediaById.get(mediaId);
      if (media && importedMediaIds.has(mediaId) &&
        !completion.problems.some((problem) => problem.fileName === media.name)) {
        problems.push({ fileName: media.name,
          reason: "Não foi possível carregar a miniatura na interface." });
      }
    };
    const prepare = async () => {
      let previews: readonly MediaPreview[] = [];
      try {
        // Import owns a single publication; partial channel events stay private.
        previews = await mediaPreviewPort.prepareMediaPreviews(request, () => undefined) ?? [];
      } catch (error: unknown) {
        logger.write({ level: "warn", component: "media-preview",
          event: "media_preview_failed", projectId, reason: logReasonFromError(error) });
      }
      if (importPresentation.current !== batch) return problems;
      const prepared = new Map(previews.map((preview) => [preview.mediaId, preview]));
      await Promise.all(demand.visibleMediaIds.map(async (mediaId) => {
        const preview = prepared.get(mediaId);
        if (preview && preview.state !== "ready") {
          reportUnavailablePreview(mediaId);
          return;
        }
        try {
          if (!preview?.url) throw new Error("Prévia não recebida.");
          await decodeMediaPreview(preview.url);
        } catch {
          prepared.set(mediaId, { mediaId, state: "cache_unavailable", url: null });
          reportUnavailablePreview(mediaId);
        }
      }));
      if (importPresentation.current !== batch) return problems;
      const nextPreviews = Object.fromEntries(prepared);
      setMediaPreviews((current) => ({ ...current, ...nextPreviews }));
      preparedPresentation.current = {
        projectId, demand, refreshRevision: mediaRefreshRevision, previews: nextPreviews,
      };
      batch.ready = true;
      return problems;
    };
    return Promise.race([prepare(), cancelled]);
  }, [logger, mediaPreviewPort, mediaRefreshRevision, projectId]);
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
        const refresh = ++latestProjectionRefresh;
        const operationId = createLogInstanceId("media-refresh");
        // The monitor can observe the committed catalog before its Cache batch
        // finishes. Publish it only when the mutation queue has settled.
        void runProjectMutation.waitForIdle().then(async () => {
          if (!active || refresh !== latestProjectionRefresh) return null;
          setMediaRefreshRevision((revision) => revision + 1);
          return projectCorePort.load(operationId);
        }).then(
          async (refreshed) => {
            // A subsequent mutation may have started while the read was in flight.
            // Its result owns equal-revision changes too, such as Save clearing dirty.
            const settled = await runProjectMutation.waitForIdle();
            if (
              refreshed &&
              settled?.status === "completed" &&
              settled.projection.state.revision >= refreshed.state.revision
            ) {
              refreshed = settled.projection;
            }
            if (
              !refreshed ||
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
  }, [logger, mediaPreviewPort, projectCorePort, projectId, runProjectMutation]);

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
    projectStartupPort.confirmUiReady().then((problems) => {
      if (uiReadyProject.current === projectId && problems) setInitialImageProblems(problems);
    }).catch((error: unknown) => {
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
      !mediaChangeListenerReady ||
      importPresentation.current
    ) {
      return;
    }
    if (mediaDemandSequence.current.projectId !== projectId) {
      mediaDemandSequence.current = { projectId, revision: 0 };
    }
    const effectiveDemand = editorGraphics.supported
      ? mediaDemand
      : { visibleMediaIds: [], preloadMediaIds: [] };
    const prepared = preparedPresentation.current;
    if (prepared?.projectId === projectId &&
      prepared.refreshRevision === mediaRefreshRevision &&
      sameMediaDemand(prepared.demand, effectiveDemand)) return;
    preparedPresentation.current = null;
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
    let completed = false;
    const operationId = createLogInstanceId("media-preview");
    logger.write({
      level: "info",
      component: "media-preview",
      event: "media_preview_started",
      operationId,
      projectId,
    });
    mediaPreviewPort
      .prepareMediaPreviews(demand, (preview) => {
        if (!active || completed || demand.revision !== mediaDemandSequence.current.revision) return;
        setMediaPreviews((current) =>
          active ? { ...current, [preview.mediaId]: preview } : current,
        );
      })
      .then((previews) => {
        if (!active || demand.revision !== mediaDemandSequence.current.revision) return;
        completed = true;
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
        completed = true;
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
          photoshopPort={photoshopPort}
          mediaDropPort={mediaDropPort}
          projection={projection}
          exportPipelinePort={exportPipelinePort}
          projectDialogPort={projectDialogPort}
          projectWindowPort={projectWindowPort}
          runProjectMutation={runProjectMutation}
          projectCorePort={projectCorePort}
          mediaPreviews={mediaPreviews}
          mediaFiles={mediaFiles}
          onMediaDemandChange={updateMediaDemand}
          prepareMediaPresentation={prepareMediaPresentation}
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
