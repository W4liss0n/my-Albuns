import { useEffect, useRef, useState } from "react";

import type { GraphicsDiagnostic } from "../application/graphics";
import { SafeApplicationShell } from "../components/SafeApplicationShell";
import type {
  GlobalProjectPort,
  OpenProjectFailure,
  OpenProjectOutcome,
  RecentProjectSummary,
} from "./application/globalProjectPort";
import { NewProjectFlow } from "./NewProjectFlow";

interface GlobalShellProps {
  graphicsDiagnostic: GraphicsDiagnostic;
  projectPort: GlobalProjectPort;
}

export function GlobalShell({
  graphicsDiagnostic,
  projectPort,
}: GlobalShellProps) {
  const [isOpening, setIsOpening] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [externalCopyPending, setExternalCopyPending] = useState(false);
  const [failure, setFailure] = useState<OpenProjectFailure | null>(null);
  const [recentProjects, setRecentProjects] = useState<
    readonly RecentProjectSummary[]
  >([]);
  const openingAttempt = useRef(0);
  const graphicsGateReported = useRef(false);

  useEffect(() => {
    if (graphicsGateReported.current) return;
    graphicsGateReported.current = true;
    void projectPort
      .completeGraphicsGate(graphicsDiagnostic.supported)
      .then((outcome) => {
        if (outcome?.status === "failed") {
          setFailure(outcome.error);
        }
      });
  }, [graphicsDiagnostic.supported, projectPort]);

  useEffect(() => {
    let active = true;
    const startupAttempt = openingAttempt.current;
    void projectPort.listRecentProjects().then((projects) => {
      if (active) {
        setRecentProjects(projects);
      }
    });
    void projectPort.startupOpenFailure().then((startupFailure) => {
      if (
        active &&
        startupFailure &&
        openingAttempt.current === startupAttempt
      ) {
        setFailure(startupFailure);
      }
    });
    return () => {
      active = false;
    };
  }, [projectPort]);

  const runOpening = async (
    attempt: () => Promise<OpenProjectOutcome>,
  ) => {
    openingAttempt.current += 1;
    setIsOpening(true);
    setFailure(null);
    setExternalCopyPending(false);
    const outcome = await attempt();
    if (outcome.status === "failed") {
      setFailure(outcome.error);
    } else if (outcome.status === "externalCopyNotWritable") {
      setExternalCopyPending(true);
    }
    setIsOpening(false);
  };

  const openProject = () => runOpening(() => projectPort.openProject());

  const openRecentProject = (id: string) =>
    runOpening(() => projectPort.openRecentProject(id));

  const saveExternalCopyAs = async () => {
    setIsOpening(true);
    setFailure(null);
    const outcome = await projectPort.saveExternalCopyAs();
    if (outcome.status === "failed") {
      setFailure(outcome.error);
    }
    setExternalCopyPending(false);
    setIsOpening(false);
  };

  const startCreation = () => {
    openingAttempt.current += 1;
    setFailure(null);
    setIsCreating(true);
  };

  if (!graphicsDiagnostic.supported) {
    return <SafeApplicationShell diagnostic={graphicsDiagnostic} />;
  }

  return (
    <>
      <main className="global-shell">
        <header className="global-shell-header">
          <span aria-hidden="true" className="global-brand-mark">
            M
          </span>
          <span>MyAlbuns</span>
        </header>

        <section className="global-recent-projects">
          <p className="global-eyebrow">Boas-vindas</p>
          <h1>Projetos recentes</h1>
          {recentProjects.length === 0 ? (
            <p>Os Projetos abertos recentemente aparecerão aqui.</p>
          ) : (
            <ul aria-label="Projetos recentes" className="global-recent-list">
              {recentProjects.map((project) => (
                <li key={project.id}>
                  <button
                    disabled={isOpening || isCreating}
                    onClick={() => openRecentProject(project.id)}
                    type="button"
                  >
                    {project.name}
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <aside aria-label="Ações principais" className="global-primary-actions">
          <button
            disabled={isOpening || isCreating}
            onClick={startCreation}
            type="button"
          >
            Novo Projeto
          </button>
          <button
            className="global-secondary-action"
            disabled={isOpening || isCreating}
            onClick={openProject}
            type="button"
          >
            {isOpening
              ? "Abrindo Projeto…"
              : failure
                ? "Tentar novamente"
                : "Abrir Projeto"}
          </button>
          {isOpening ? (
            <p aria-live="polite" role="status">
              Preparando a Janela do Projeto…
            </p>
          ) : null}
          {failure ? (
            <section className="global-open-error" role="alert">
              <h2>Não foi possível abrir o Projeto</h2>
              <p>{failure.message}</p>
              {failure.action ? <p>{failure.action}</p> : null}
            </section>
          ) : null}
          {externalCopyPending ? (
            <section className="global-copy-resolution">
              <h2>Cópia externa somente leitura</h2>
              <p>
                O arquivo original será preservado. Escolha outro local para
                criar uma cópia editável com Identidade própria.
              </p>
              <button
                disabled={isOpening || isCreating}
                onClick={() => void saveExternalCopyAs()}
                type="button"
              >
                {isOpening ? "Salvando cópia…" : "Salvar cópia como…"}
              </button>
            </section>
          ) : null}
        </aside>
      </main>
      {isCreating ? (
        <NewProjectFlow
          onCancel={() => {
            void projectPort.clearProvisionalDecoratives();
            setIsCreating(false);
          }}
          onChooseDecorative={() =>
            projectPort.chooseProvisionalDecorative()
          }
          onCreate={(configuration) =>
            projectPort.createProject(configuration)
          }
          onReleaseDecorative={(selectionId) =>
            projectPort.releaseProvisionalDecorative(selectionId)
          }
          onValidate={(configuration) =>
            projectPort.validateProjectConfiguration(configuration)
          }
        />
      ) : null}
    </>
  );
}
