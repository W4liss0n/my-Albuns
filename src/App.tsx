import { useEffect, useMemo, useState } from "react";

import {
  tauriProjectBridge,
  type EditorProjection,
  type ProjectBridge,
} from "./domain/project";
import {
  probeGraphics,
  type GraphicsDiagnostic,
} from "./platform/graphics";
import { ProjectWorkspace } from "./components/ProjectWorkspace";
import "./App.css";

interface AppProps {
  bridge?: ProjectBridge;
  graphicsProbe?: () => GraphicsDiagnostic;
}

function App({
  bridge = tauriProjectBridge,
  graphicsProbe = probeGraphics,
}: AppProps) {
  const graphics = useMemo(() => graphicsProbe(), [graphicsProbe]);
  const [projection, setProjection] = useState<EditorProjection | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    bridge
      .load()
      .then((value) => {
        if (active) setProjection(value);
      })
      .catch((error: unknown) => {
        if (active) {
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
  }, [bridge]);

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
    <ProjectWorkspace
      projection={projection}
      bridge={bridge}
      onProjectionChange={setProjection}
    />
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
