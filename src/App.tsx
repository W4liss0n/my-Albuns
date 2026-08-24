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
  ExportPort,
  MediaPreview,
  MediaPreviewDemand,
  MediaPreviewPort,
  ProjectStartupPort,
  ProjectSessionPort,
  ProjectWindowPort,
} from "./application/projectPorts";
import type { ProjectDialogPort } from "./application/projectDialogPort";
import type { WorkspacePreferencesPort } from "./application/workspacePreferences";
import type { EditorProjection } from "./domain/project";
import { LoggingProvider } from "./components/loggingContext";
import {
  CanvasGraphicsDiagnosticProbeProvider,
  type CanvasGraphicsDiagnosticProbe,
} from "./components/canvasGraphicsDiagnosticProbeContext";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import { useProjectMutationRunner } from "./components/useProjectMutationRunner";
import { BrandWordmark, InlineNotice } from "./ui";
import "./App.css";

interface AppProps {
  exportPort: ExportPort;
  mediaPreviewPort: MediaPreviewPort;
  projectStartupPort: ProjectStartupPort;
  projectSessionPort: ProjectSessionPort;
  projectDialogPort: ProjectDialogPort;
  projectWindowPort: ProjectWindowPort;
  graphicsProbe: GraphicsProbe;
  canvasGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe;
  logger: Logger;
  workspacePreferencesPort?: WorkspacePreferencesPort;
}

function App({
  exportPort,
  mediaPreviewPort,
  projectStartupPort,
  projectSessionPort,
  projectDialogPort,
  projectWindowPort,
  graphicsProbe,
  canvasGraphicsDiagnosticProbe,
  logger,
  workspacePreferencesPort,
}: AppProps) {
  const graphics = useMemo(() => graphicsProbe(), [graphicsProbe]);
  const [runtimeGraphicsDiagnostic, setRuntimeGraphicsDiagnostic] =
    useState<GraphicsDiagnostic | null>(null);
  const editorGraphics = runtimeGraphicsDiagnostic ?? graphics;
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
  const mediaDemandSequence = useRef({ projectId: "", revision: 0 });
  const uiReadyProject = useRef("");
  const [preferencesReadyProject, setPreferencesReadyProject] = useState("");

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
    projectSessionPort
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
  }, [graphics.supported, logger, projectSessionPort]);

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
  useEffect(() => {
    if (
      !projectId ||
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
  }, [logger, preferencesReadyProject, projectId, projectStartupPort]);
  const runProjectMutation = useProjectMutationRunner(
    projectId,
    projectSessionPort,
  );
  useEffect(() => {
    if (!projectId) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void mediaPreviewPort
      .onMediaChanged(() => {
        if (!active) return;
        setMediaRefreshRevision((revision) => revision + 1);
      })
      .then((dispose) => {
        if (active) {
          unlisten = dispose;
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
    };
  }, [logger, mediaPreviewPort, projectId]);

  useEffect(() => {
    setMediaPreviews({});
  }, [projectId]);

  useEffect(() => {
    if (!projectId) return;
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
    editorGraphics.supported,
    logger,
    mediaDemand,
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
          <BrandWordmark compact />
          <p className="eyebrow">MyAlbuns</p>
          <h1>Não foi possível abrir o Projeto</h1>
          <InlineNotice tone="error">{loadError}</InlineNotice>
        </section>
      </main>
    );
  }

  if (!projection) {
    return (
      <main className="startup-surface" aria-busy="true">
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
          exportPort={exportPort}
          projectDialogPort={projectDialogPort}
          projectWindowPort={projectWindowPort}
          runProjectMutation={runProjectMutation}
          validateAlbumInformation={projectSessionPort.validateAlbumInformation}
          mediaPreviews={mediaPreviews}
          onMediaDemandChange={setMediaDemand}
          onProjectionChange={setProjection}
          onGraphicsUnavailable={setRuntimeGraphicsDiagnostic}
          onPreferencesReady={handlePreferencesReady}
          workspacePreferencesPort={workspacePreferencesPort}
        />
      </CanvasGraphicsDiagnosticProbeProvider>
    </LoggingProvider>
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
        <BrandWordmark compact />
        <p className="eyebrow">Editor indisponível</p>
        <h1>O Canvas não pôde ser iniciado</h1>
        <InlineNotice tone="error">{diagnostic.reason}</InlineNotice>
        <InlineNotice className="support-note">
          Feche esta Janela do Projeto e use o Diagnóstico gráfico da
          Boas-vindas antes de tentar novamente.
        </InlineNotice>
      </section>
    </main>
  );
}

export default App;
