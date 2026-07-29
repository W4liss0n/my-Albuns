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
import {
  type EditorProjection,
  type ProjectBridge,
} from "./domain/project";
import { LoggingProvider } from "./components/loggingContext";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import "./App.css";

interface AppProps {
  bridge: ProjectBridge;
  graphicsProbe: GraphicsProbe;
  logger: Logger;
}

function App({ bridge, graphicsProbe, logger }: AppProps) {
  const graphics = useMemo(() => graphicsProbe(), [graphicsProbe]);
  const [projection, setProjection] = useState<EditorProjection | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [mediaPreviewUrls, setMediaPreviewUrls] = useState<
    Readonly<Record<string, string>>
  >({});

  useEffect(() => {
    let active = true;
    const operationId = createLogInstanceId("project-load");
    logger.write({
      level: "info",
      component: "application",
      event: "project_load_started",
      operationId,
    });
    bridge
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
  }, [bridge, logger]);

  useEffect(() => {
    logger.write({
      level: graphics.supported ? "info" : "warn",
      component: "graphics",
      event: graphics.supported
        ? "graphics_probe_succeeded"
        : "graphics_probe_failed",
      reason: graphics.supported
        ? undefined
        : "hardware_acceleration_unavailable",
    });
  }, [graphics, logger]);

  const projectId = projection?.state.projectId;
  useEffect(() => {
    setMediaPreviewUrls({});
    if (!projectId || !graphics.supported) return;

    let active = true;
    const operationId = createLogInstanceId("media-cache");
    logger.write({
      level: "info",
      component: "media-cache",
      event: "media_cache_started",
      operationId,
      projectId,
    });
    bridge
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
  }, [bridge, graphics.supported, logger, projectId]);

  if (!graphics.supported) {
    return <GraphicsUnavailable diagnostic={graphics} />;
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
      <ProjectWorkspace
        projection={projection}
        bridge={bridge}
        mediaPreviewUrls={mediaPreviewUrls}
        onProjectionChange={setProjection}
      />
    </LoggingProvider>
  );
}

function GraphicsUnavailable({
  diagnostic,
}: {
  diagnostic: GraphicsDiagnostic;
}) {
  return (
    <main className="startup-surface">
      <section className="startup-card diagnostic-card">
        <div className="brand-lockup" aria-label="MyAlbuns">
          <span className="brand-mark" aria-hidden="true">
            M
          </span>
          <span>MyAlbuns</span>
        </div>
        <p className="eyebrow">Diagnóstico gráfico</p>
        <h1>Editor indisponível neste computador</h1>
        <p>{diagnostic.reason}</p>
        <dl className="diagnostic-list">
          <div>
            <dt>Backend detectado</dt>
            <dd>{diagnostic.renderer}</dd>
          </div>
          <div>
            <dt>Requisito</dt>
            <dd>WebGL2 com aceleração por hardware</dd>
          </div>
        </dl>
        <p className="support-note">
          O diagnóstico permanece disponível. Reative a aceleração por
          hardware para abrir o editor com desempenho e composição visual
          consistentes.
        </p>
      </section>
    </main>
  );
}

export default App;
