import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import {
  ChevronRight,
  Download,
  FolderOpen,
  Plus,
  Star,
} from "lucide-react";

import type { GraphicsDiagnostic } from "../application/graphics";
import {
  matchProjectCommandShortcut,
  projectCommandShortcutAria,
  projectCommandShortcutLabel,
} from "../application/projectCommandCatalog";
import { SafeApplicationShell } from "../components/SafeApplicationShell";
import type {
  GlobalProjectPort,
  NewProjectPort,
  OpenProjectOutcome,
  ProjectFailureDialogPort,
  RecentProjectSummary,
} from "./application/globalProjectPort";
import { NewProjectFlow } from "./NewProjectFlow";
import {
  ActionButton,
  AppIcon,
  ApplicationHeader,
  BrandWordmark,
  EmptyState,
} from "../ui";

interface GlobalShellProps {
  failureDialogPort: ProjectFailureDialogPort;
  graphicsDiagnostic: GraphicsDiagnostic;
  newProjectPort: NewProjectPort;
  projectPort: GlobalProjectPort;
}

const recentCoverVariants = [1, 2, 1, 3, 4, 1, 2] as const;
const portraitCoverIndexes = new Set([1, 4, 6]);

export function GlobalShell({
  failureDialogPort,
  graphicsDiagnostic,
  newProjectPort,
  projectPort,
}: GlobalShellProps) {
  const [isOpening, setIsOpening] = useState(false);
  const [surface, setSurface] = useState<"welcome" | "newProject">(
    "welcome",
  );
  const [recentProjects, setRecentProjects] = useState<
    readonly RecentProjectSummary[]
  >([]);
  const openingAttempt = useRef(0);
  const graphicsGateReported = useRef(false);
  const newProjectTriggerRef = useRef<HTMLButtonElement>(null);
  const restoreNewProjectTriggerFocus = useRef(false);

  useEffect(() => {
    if (graphicsGateReported.current) return;
    graphicsGateReported.current = true;
    void projectPort
      .completeGraphicsGate(graphicsDiagnostic.supported)
      .then((outcome) => {
        if (outcome?.status === "failed") {
          void failureDialogPort.present({
            context: "projectOpening",
            error: outcome.error,
          });
        }
      });
  }, [failureDialogPort, graphicsDiagnostic.supported, projectPort]);

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
        void failureDialogPort.present({
          context: "projectOpening",
          error: startupFailure,
        });
      }
    });
    return () => {
      active = false;
    };
  }, [failureDialogPort, projectPort]);

  const runOpening = useCallback(
    async (attempt: () => Promise<OpenProjectOutcome>) => {
      openingAttempt.current += 1;
      setIsOpening(true);
      const outcome = await attempt();
      if (outcome.status === "failed") {
        await failureDialogPort.present({
          context: "projectOpening",
          error: outcome.error,
        });
      }
      setIsOpening(false);
    },
    [failureDialogPort],
  );

  const openProject = useCallback(
    () => runOpening(() => projectPort.openProject()),
    [projectPort, runOpening],
  );

  const openRecentProject = (id: string) =>
    runOpening(() => projectPort.openRecentProject(id));

  const startCreation = useCallback(() => {
    openingAttempt.current += 1;
    setSurface("newProject");
  }, []);

  const cancelCreation = useCallback(() => {
    restoreNewProjectTriggerFocus.current = true;
    setSurface("welcome");
  }, []);

  useLayoutEffect(() => {
    if (
      surface !== "welcome" ||
      !restoreNewProjectTriggerFocus.current
    ) {
      return;
    }
    restoreNewProjectTriggerFocus.current = false;
    newProjectTriggerRef.current?.focus({ preventScroll: true });
  }, [surface]);

  useEffect(() => {
    if (
      !graphicsDiagnostic.supported ||
      isOpening ||
      surface !== "welcome"
    ) {
      return;
    }

      const handleShortcut = (event: KeyboardEvent) => {
      if (event.repeat) {
        return;
      }

      const command = matchProjectCommandShortcut(event, "welcome");
      if (command === "new-project") {
        event.preventDefault();
        startCreation();
      } else if (command === "open-project") {
        event.preventDefault();
        void openProject();
      }
    };

    window.addEventListener("keydown", handleShortcut);
    return () => window.removeEventListener("keydown", handleShortcut);
  }, [
    graphicsDiagnostic.supported,
    isOpening,
    openProject,
    startCreation,
    surface,
  ]);

  if (!graphicsDiagnostic.supported) {
    return <SafeApplicationShell diagnostic={graphicsDiagnostic} />;
  }

  if (surface === "newProject") {
    return (
      <div className="global-shell global-shell--new-project">
        <ApplicationHeader context="Novo Projeto" />
        <NewProjectFlow
          onCancel={cancelCreation}
          onChooseDecorative={() =>
            newProjectPort.chooseProvisionalDecorative()
          }
          onCreate={(configuration) =>
            newProjectPort.createProject(configuration)
          }
          onOperationalFailure={(failure) =>
            failureDialogPort.present(failure)
          }
          onReleaseDecorative={(selectionId) =>
            newProjectPort.releaseProvisionalDecorative(selectionId)
          }
          onValidate={(configuration) =>
            newProjectPort.validateProjectConfiguration(configuration)
          }
        />
      </div>
    );
  }

  return (
    <div className="global-shell">
      <ApplicationHeader status="diagramação de Álbuns" />

        <main className="global-recent-projects">
          <h1 className="ui-section-eyebrow">Projetos recentes</h1>
          {recentProjects.length === 0 ? (
            <EmptyState
              className="global-empty-state"
              description="Os Projetos abertos recentemente aparecerão aqui."
              icon={<AppIcon icon={FolderOpen} size={16} />}
              title="Nenhum Projeto recente"
            />
          ) : (
            <ul
              aria-label="Projetos recentes"
              className="global-recent-list"
              data-placeholder-feature="recent-project-visual-metadata"
            >
              {/* PLACEHOLDER UI: capas, fixação e metadados secundários vêm da
                  referência visual, mas ainda não existem no contrato de recentes. */}
              {recentProjects.map((project, index) => (
                <li key={project.id}>
                  <button
                    aria-label={project.name}
                    disabled={isOpening}
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
        </main>

        <aside aria-label="Ações principais" className="global-primary-actions">
          <BrandWordmark subtitle="diagramação de Álbuns · versão 0.1.0" />
          <div className="global-action-stack">
            <ActionButton
              aria-label="Novo Projeto"
              aria-keyshortcuts={projectCommandShortcutAria("new-project")}
              disabled={isOpening}
              onClick={startCreation}
              ref={newProjectTriggerRef}
              variant="primary"
            >
              <AppIcon icon={Plus} size={16} />
              <span>Novo Projeto</span>
              <kbd>{projectCommandShortcutLabel("new-project")}</kbd>
            </ActionButton>
            <ActionButton
              aria-label={isOpening ? "Abrindo Projeto…" : "Abrir Projeto"}
              aria-keyshortcuts={projectCommandShortcutAria("open-project")}
              disabled={isOpening}
              onClick={openProject}
            >
              <AppIcon icon={FolderOpen} size={16} />
              <span>
                {isOpening ? "Abrindo Projeto…" : "Abrir Projeto…"}
              </span>
              <kbd>{projectCommandShortcutLabel("open-project")}</kbd>
            </ActionButton>
          </div>
          <div aria-hidden="true" className="global-action-divider" />
          <div className="global-secondary-actions">
            {/* PLACEHOLDER UI: ainda não existe uma porta de Exportação em lote. */}
            <button
              aria-label="Exportação em lote"
              data-placeholder-feature="batch-export"
              disabled
              title="A Exportação em lote ainda não está disponível"
              type="button"
            >
              <AppIcon icon={Download} size={14} />
              <span>Exportação em lote</span>
            </button>
          </div>
        </aside>
    </div>
  );
}
