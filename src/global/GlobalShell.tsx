import { useCallback, useEffect, useRef, useState } from "react";

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

interface LaunchViewState {
  externalCopyPending: boolean;
  failure: OpenProjectFailure | null;
}

type LaunchOutcomeContext = "opening" | "externalCopyResolution";

function reduceLaunchOutcome(
  state: LaunchViewState,
  outcome: OpenProjectOutcome,
  context: LaunchOutcomeContext,
): LaunchViewState {
  switch (outcome.status) {
    case "failed":
      return {
        externalCopyPending:
          context === "externalCopyResolution"
            ? false
            : state.externalCopyPending,
        failure: outcome.error,
      };
    case "externalCopyNotWritable":
      return { externalCopyPending: true, failure: null };
    case "opened":
    case "focused":
      return { externalCopyPending: false, failure: null };
    case "cancelled":
      return context === "externalCopyResolution"
        ? { externalCopyPending: false, failure: null }
        : state;
  }
}

export function GlobalShell({
  graphicsDiagnostic,
  projectPort,
}: GlobalShellProps) {
  const [isOpening, setIsOpening] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
  const [{ externalCopyPending, failure }, setLaunchView] =
    useState<LaunchViewState>({
      externalCopyPending: false,
      failure: null,
    });
  const [recentProjects, setRecentProjects] = useState<
    readonly RecentProjectSummary[]
  >([]);
  const openingAttempt = useRef(0);
  const graphicsGateReported = useRef(false);
  const applyLaunchOutcome = useCallback(
    (
      outcome: OpenProjectOutcome,
      context: LaunchOutcomeContext = "opening",
    ) => {
      setLaunchView((state) =>
        reduceLaunchOutcome(state, outcome, context),
      );
    },
    [],
  );

  useEffect(() => {
    if (graphicsGateReported.current) return;
    graphicsGateReported.current = true;
    const graphicsAttempt = openingAttempt.current;
    void projectPort
      .completeGraphicsGate(graphicsDiagnostic.supported)
      .then((outcome) => {
        if (openingAttempt.current !== graphicsAttempt) return;
        if (outcome) applyLaunchOutcome(outcome);
      });
  }, [applyLaunchOutcome, graphicsDiagnostic.supported, projectPort]);

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
        setLaunchView((state) => ({
          ...state,
          failure: startupFailure,
        }));
      }
    });
    return () => {
      active = false;
    };
  }, [projectPort]);

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void projectPort
      .onActivationTerminal((outcome) => {
        if (!active) return;
        openingAttempt.current += 1;
        setIsOpening(false);
        applyLaunchOutcome(outcome);
      })
      .then((release) => {
        if (active) {
          unlisten = release;
        } else {
          release();
        }
      })
      .catch(() => {
        // Direct commands still surface failures if the event channel is unavailable.
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [applyLaunchOutcome, projectPort]);

  const runOpening = async (
    attempt: () => Promise<OpenProjectOutcome>,
  ) => {
    openingAttempt.current += 1;
    setIsOpening(true);
    setLaunchView((state) => ({ ...state, failure: null }));
    const outcome = await attempt();
    applyLaunchOutcome(outcome);
    setIsOpening(false);
  };

  const openProject = () => runOpening(() => projectPort.openProject());

  const openRecentProject = (id: string) =>
    runOpening(() => projectPort.openRecentProject(id));

  const saveExternalCopyAs = async () => {
    setIsOpening(true);
    setLaunchView((state) => ({ ...state, failure: null }));
    const outcome = await projectPort.saveExternalCopyAs();
    applyLaunchOutcome(outcome, "externalCopyResolution");
    setIsOpening(false);
  };

  const startCreation = () => {
    openingAttempt.current += 1;
    setLaunchView((state) => ({ ...state, failure: null }));
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
