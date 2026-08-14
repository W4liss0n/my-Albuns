import { useEffect, useRef, useState } from "react";
import {
  ChevronRight,
  Download,
  FolderOpen,
  Plus,
  Star,
} from "lucide-react";

import type { GraphicsDiagnostic } from "../application/graphics";
import { SafeApplicationShell } from "../components/SafeApplicationShell";
import type {
  GlobalProjectPort,
  OpenProjectOutcome,
  RecentProjectSummary,
} from "./application/globalProjectPort";
import {
  ActionButton,
  AppIcon,
  ApplicationHeader,
  BrandWordmark,
} from "../ui";

interface GlobalShellProps {
  graphicsDiagnostic: GraphicsDiagnostic;
  projectPort: GlobalProjectPort;
}

const recentCoverVariants = [1, 2, 1, 3, 4, 1, 2] as const;
const portraitCoverIndexes = new Set([1, 4, 6]);

export function GlobalShell({
  graphicsDiagnostic,
  projectPort,
}: GlobalShellProps) {
  const [isOpening, setIsOpening] = useState(false);
  const [isCreating, setIsCreating] = useState(false);
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
          void projectPort.showLaunchFailure(outcome.error);
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
        void projectPort.showLaunchFailure(startupFailure);
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
    const outcome = await attempt();
    if (outcome.status === "failed") {
      await projectPort.showLaunchFailure(outcome.error);
    }
    setIsOpening(false);
  };

  const openProject = () => runOpening(() => projectPort.openProject());

  const openRecentProject = (id: string) =>
    runOpening(() => projectPort.openRecentProject(id));

  const startCreation = async () => {
    openingAttempt.current += 1;
    setIsCreating(true);
    const outcome = await projectPort.showNewProjectWindow();
    if (outcome.status === "failed") {
      await projectPort.showLaunchFailure(outcome.error);
    }
    setIsCreating(false);
  };

  if (!graphicsDiagnostic.supported) {
    return <SafeApplicationShell diagnostic={graphicsDiagnostic} />;
  }

  return (
    <main className="global-shell">
      <ApplicationHeader status="diagramação de Álbuns" />

        <section className="global-recent-projects">
          <div className="global-section-heading">
            <h1>Projetos recentes</h1>
          </div>
          {recentProjects.length === 0 ? (
            <div className="global-empty-state">
              <strong>Nenhum Projeto recente</strong>
              <p>Os Projetos abertos recentemente aparecerão aqui.</p>
            </div>
          ) : (
            <ul aria-label="Projetos recentes" className="global-recent-list">
              {recentProjects.map((project, index) => (
                <li key={project.id}>
                  <button
                    aria-label={project.name}
                    disabled={isOpening || isCreating}
                    onClick={() => openRecentProject(project.id)}
                    type="button"
                  >
                    <span
                      aria-hidden="true"
                      className="global-project-thumbnail"
                      data-shape={
                        portraitCoverIndexes.has(index % 7)
                          ? "portrait"
                          : "square"
                      }
                      data-variant={recentCoverVariants[index % 7]}
                    >
                      <i />
                      <span className="global-project-pin">
                        <AppIcon icon={Star} size={12} />
                      </span>
                    </span>
                    <span className="global-project-summary">
                      <strong>{project.name}</strong>
                      <small>Projeto MyAlbuns</small>
                      <small className="global-project-when">
                        Aberto recentemente
                      </small>
                    </span>
                    <span aria-hidden="true" className="global-project-open">
                      <AppIcon icon={ChevronRight} size={12} />
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          )}
        </section>

        <aside aria-label="Ações principais" className="global-primary-actions">
          <BrandWordmark subtitle="diagramação de Álbuns · versão 0.1.0" />
          <div className="global-action-stack">
            <ActionButton
              aria-label="Novo Projeto"
              disabled={isOpening || isCreating}
              onClick={() => void startCreation()}
              variant="primary"
            >
              <AppIcon icon={Plus} size={16} />
              <span>Novo Projeto</span>
              <kbd>Ctrl+N</kbd>
            </ActionButton>
            <ActionButton
              aria-label={isOpening ? "Abrindo Projeto…" : "Abrir Projeto"}
              disabled={isOpening || isCreating}
              onClick={openProject}
            >
              <AppIcon icon={FolderOpen} size={16} />
              <span>
                {isOpening ? "Abrindo Projeto…" : "Abrir Projeto…"}
              </span>
              <kbd>Ctrl+O</kbd>
            </ActionButton>
          </div>
          <div aria-hidden="true" className="global-action-divider" />
          <div className="global-secondary-actions">
            <button
              aria-label="Exportar vários Álbuns de uma vez"
              disabled
              title="A Exportação em lote ainda não está disponível"
              type="button"
            >
              <AppIcon icon={Download} size={14} />
              <span>Exportar vários Álbuns de uma vez</span>
            </button>
          </div>
        </aside>
    </main>
  );
}
