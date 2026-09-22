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
  RecentProjectFirstSheet,
} from "./application/globalProjectPort";
import { NewProjectFlow } from "./NewProjectFlow";
import { recentProjectOpeningTime } from "./recentProjectOpeningTime";
import { SheetPreviewShell } from "../components/SheetPreview";
import {
  ActionButton,
  AppIcon,
  ApplicationHeader,
  BrandWordmark,
  EmptyState,
} from "../ui";

interface GlobalShellProps {
  initialSurface?: "welcome" | "newProject";
  recentProjectsNow?: Date;
  onNewProjectRequest?(listener: () => void): Promise<() => void>;
  onOpenBatch?(): Promise<void>;
  onOpenSettings?(): Promise<void>;
  failureDialogPort: ProjectFailureDialogPort;
  graphicsDiagnostic: GraphicsDiagnostic;
  newProjectPort: NewProjectPort;
  projectPort: GlobalProjectPort;
}

function RecentProjectThumbnail({
  id, load,
}: {
  id: string;
  load(id: string): Promise<RecentProjectFirstSheet | null>;
}) {
  const element = useRef<HTMLSpanElement>(null);
  const [visible, setVisible] = useState(false);
  const [preview, setPreview] = useState<RecentProjectFirstSheet | null>(null);
  const [resolved, setResolved] = useState(false);

  useEffect(() => {
    if (!element.current) return;
    if (!window.IntersectionObserver) {
      setVisible(true);
      return;
    }
    const observer = new IntersectionObserver(([entry]) => {
      if (entry?.isIntersecting) {
        setVisible(true);
        observer.disconnect();
      }
    });
    observer.observe(element.current);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!visible) return;
    let active = true;
    void load(id).then((result) => {
      if (active) { setPreview(result); setResolved(true); }
    });
    return () => { active = false; };
  }, [id, load, visible]);

  const sheet = preview?.sheet;
  return (
    <span aria-hidden="true" className="global-project-thumbnail"
      data-preview-state={resolved ? (preview ? "ready" : "unavailable") : "loading"} ref={element}>
      {sheet ? (
        <span
          className="global-project-first-sheet"
          style={{
            aspectRatio: `${sheet.widthUm * (sheet.activeSides === "both" ? 1 : 2)} / ${sheet.heightUm}`,
            width: `${Math.round(102 * sheet.widthUm * (sheet.activeSides === "both" ? 1 : 2) / sheet.heightUm)}px`,
          }}
        >
          <SheetPreviewShell sheet={sheet} mediaPreviewUrls={preview.mediaPreviewUrls} />
        </span>
      ) : (
        <span className="global-project-album" />
      )}
    </span>
  );
}

function RecentProjectOpenedAt({ lastOpenedAtMs, now }: {
  lastOpenedAtMs: number | null;
  now: Date;
}) {
  const openedAt = recentProjectOpeningTime(lastOpenedAtMs, now);
  return openedAt ? (
    <time className="global-project-when" dateTime={openedAt.dateTime}
      aria-label={`Última abertura: ${openedAt.label}`}>
      {openedAt.label}
    </time>
  ) : null;
}

export function GlobalShell({
  initialSurface = "welcome",
  recentProjectsNow,
  onNewProjectRequest,
  onOpenSettings,
  onOpenBatch,
  failureDialogPort,
  graphicsDiagnostic,
  newProjectPort,
  projectPort,
}: GlobalShellProps) {
  const [isOpening, setIsOpening] = useState(false);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [surface, setSurface] = useState<"welcome" | "newProject">(
    initialSurface,
  );
  const [recentProjects, setRecentProjects] = useState<
    readonly RecentProjectSummary[]
  >([]);
  const openingAttempt = useRef(0);
  const graphicsGateReported = useRef(false);
  const newProjectTriggerRef = useRef<HTMLButtonElement>(null);
  const restoreNewProjectTriggerFocus = useRef(false);

  const handleLaunchOutcome = useCallback(
    async (outcome: OpenProjectOutcome) => {
      switch (outcome.status) {
        case "failed":
          await failureDialogPort.present({
            context: "projectOpening",
            error: outcome.error,
          });
          return;
        case "opened":
        case "focused":
          return;
        case "cancelled":
          return;
      }
    },
    [failureDialogPort],
  );

  useEffect(() => {
    if (graphicsGateReported.current) return;
    graphicsGateReported.current = true;
    const graphicsAttempt = openingAttempt.current;
    void projectPort
      .completeGraphicsGate(graphicsDiagnostic.supported)
      .then((outcome) => {
        if (openingAttempt.current !== graphicsAttempt || !outcome) return;
        void handleLaunchOutcome(outcome);
      });
  }, [graphicsDiagnostic.supported, handleLaunchOutcome, projectPort]);

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

  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    void projectPort
      .onActivationTerminal((outcome) => {
        if (!active) return;
        openingAttempt.current += 1;
        setIsOpening(false);
        void handleLaunchOutcome(outcome);
      })
      .then((release) => {
        if (active) {
          unlisten = release;
        } else {
          release();
        }
      })
      .catch(() => {
        // Direct commands still report their own terminal outcome.
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [handleLaunchOutcome, projectPort]);

  const runOpening = useCallback(
    async (attempt: () => Promise<OpenProjectOutcome>) => {
      openingAttempt.current += 1;
      setIsOpening(true);
      const outcome = await attempt();
      await handleLaunchOutcome(outcome);
      setIsOpening(false);
    },
    [handleLaunchOutcome],
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

  useEffect(() => {
    if (!onNewProjectRequest) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    void onNewProjectRequest(() => { if (active) startCreation(); })
      .then((release) => { if (active) unlisten = release; else release(); })
      .catch(() => {
        if (active) void failureDialogPort.present({ context: "projectCreation", error: {
          code: "new_project_activation_unavailable",
          message: "Não foi possível receber o pedido de Novo projeto.",
          action: "Use Novo projeto nesta janela ou tente novamente.",
        } });
      });
    return () => { active = false; unlisten?.(); };
  }, [onNewProjectRequest, startCreation, failureDialogPort]);

  const cancelCreation = useCallback(() => {
    void newProjectPort.clearProvisionalDecoratives();
    restoreNewProjectTriggerFocus.current = true;
    setSurface("welcome");
  }, [newProjectPort]);

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
    return <SafeApplicationShell diagnostic={graphicsDiagnostic} onOpenSettings={onOpenSettings} />;
  }

  if (surface === "newProject") {
    return (
      <div className="global-shell global-shell--new-project ui-chrome-selection-scope">
        <ApplicationHeader context="Novo projeto" />
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

  const recentNow = recentProjectsNow ?? new Date();

  return (
    <div className="global-shell ui-chrome-selection-scope">
      <ApplicationHeader status="diagramação de álbuns" />

      <main className="global-recent-projects">
        <h1 className="ui-section-eyebrow">Projetos recentes</h1>
        {recentProjects.length === 0 ? (
          <EmptyState
            className="global-empty-state"
            description="Crie um projeto ou abra um arquivo .myalbuns."
            title="Nenhum projeto recente"
          />
        ) : (
          <ul
            aria-label="Projetos recentes"
            className="global-recent-list"
          >
            {recentProjects.map((project) => (
              <li key={project.id}>
                <button
                  aria-label={project.name}
                  disabled={isOpening}
                  onClick={() => openRecentProject(project.id)}
                  type="button"
                >
                  <RecentProjectThumbnail id={project.id} load={projectPort.firstRecentProjectSheet} />
                  <span className="global-project-summary">
                    <strong>{project.name}</strong>
                    <RecentProjectOpenedAt lastOpenedAtMs={project.lastOpenedAtMs} now={recentNow} />
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
        <BrandWordmark subtitle="diagramação de álbuns · versão 0.1.0" />
        <div className="global-action-stack">
          <ActionButton
            aria-label="Novo projeto"
            aria-keyshortcuts={projectCommandShortcutAria("new-project")}
            disabled={isOpening}
            onClick={startCreation}
            ref={newProjectTriggerRef}
            variant="primary"
          >
            <AppIcon icon={Plus} size={16} />
            <span>Novo projeto</span>
            <kbd>{projectCommandShortcutLabel("new-project")}</kbd>
          </ActionButton>
          <ActionButton
            aria-label={isOpening ? "Abrindo projeto…" : "Abrir projeto"}
            aria-keyshortcuts={projectCommandShortcutAria("open-project")}
            disabled={isOpening}
            onClick={openProject}
          >
            <AppIcon icon={FolderOpen} size={16} />
            <span>
              {isOpening ? "Abrindo projeto…" : "Abrir projeto…"}
            </span>
            <kbd>{projectCommandShortcutLabel("open-project")}</kbd>
          </ActionButton>
        </div>
        <div aria-hidden="true" className="global-action-divider" />
        <div className="global-secondary-actions">
          <button type="button" disabled={isOpening || !onOpenSettings} onClick={() => {
            setSettingsError(null);
            void onOpenSettings?.().catch(() => setSettingsError("Não foi possível abrir Configurações. Tente novamente."));
          }}>Configurações…</button>
          {settingsError && <p role="alert">{settingsError}</p>}
          <button
            aria-label="Exportação em lote"
            disabled={isOpening || !onOpenBatch}
            onClick={() => { void onOpenBatch?.().catch(() => setSettingsError("Não foi possível abrir a exportação em lote.")); }}
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
