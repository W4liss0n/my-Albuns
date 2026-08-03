import { useEffect, useMemo, useState } from "react";

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
  MediaPreviewPort,
  ProjectSessionPort,
} from "./application/projectPorts";
import type { EditorProjection } from "./domain/project";
import { LoggingProvider } from "./components/loggingContext";
import {
  CanvasGraphicsDiagnosticProbeProvider,
  type CanvasGraphicsDiagnosticProbe,
} from "./components/canvasGraphicsDiagnosticProbeContext";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import { SafeApplicationShell } from "./components/SafeApplicationShell";
import { useProjectMutationRunner } from "./components/useProjectMutationRunner";
import "./App.css";

interface AppProps {
  exportPort: ExportPort;
  mediaPreviewPort: MediaPreviewPort;
  projectSessionPort: ProjectSessionPort;
  graphicsProbe: GraphicsProbe;
  canvasGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe;
  logger: Logger;
}

function App({
  exportPort,
  mediaPreviewPort,
  projectSessionPort,
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
  const [mediaPreviewUrls, setMediaPreviewUrls] = useState<
    Readonly<Record<string, string>>
  >({});

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
  const runProjectMutation = useProjectMutationRunner(
    projectId,
    projectSessionPort,
  );
  useEffect(() => {
    setMediaPreviewUrls({});
    if (!projectId || !editorGraphics.supported) return;

    let active = true;
    const operationId = createLogInstanceId("media-cache");
    logger.write({
      level: "info",
      component: "media-cache",
      event: "media_cache_started",
      operationId,
      projectId,
    });
    mediaPreviewPort
      .prepareMediaPreviews()
      .then((previews) => {
        if (!active) return;
        if (previews) {
          setMediaPreviewUrls(
            Object.fromEntries(
              previews.map(({ mediaId, url }) => [
                mediaId,
                url,
              ]),
            ),
          );
        }
        logger.write({
          level: "info",
          component: "media-cache",
          event: "media_cache_completed",
          operationId,
          projectId,
        });
      })
      .catch((error: unknown) => {
        if (!active) return;
        logger.write({
          level: "warn",
          component: "media-cache",
          event: "media_cache_failed",
          operationId,
          projectId,
          reason: logReasonFromError(error),
        });
      });
    return () => {
      active = false;
    };
  }, [editorGraphics.supported, logger, mediaPreviewPort, projectId]);

  if (!editorGraphics.supported) {
    return <SafeApplicationShell diagnostic={editorGraphics} />;
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
        <ProjectWorkspace
          projection={projection}
          exportPort={exportPort}
          runProjectMutation={runProjectMutation}
          mediaPreviewUrls={mediaPreviewUrls}
          onProjectionChange={setProjection}
          onGraphicsUnavailable={setRuntimeGraphicsDiagnostic}
        />
      </CanvasGraphicsDiagnosticProbeProvider>
    </LoggingProvider>
  );
}

export default App;
