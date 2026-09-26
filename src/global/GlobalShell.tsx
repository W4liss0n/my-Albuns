import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { createPortal } from "react-dom";
import { useHover, useTooltip, useTooltipTrigger } from "react-aria";
import { useTooltipTriggerState } from "react-stately";
import {
  Download,
  FolderOpen,
  Plus,
  Settings,
  Star,
} from "lucide-react";

import type { GraphicsDiagnostic } from "../application/graphics";
import {
  matchProjectCommandShortcut,
  projectCommandShortcutAria,
  projectCommandShortcutLabel,
} from "../application/projectCommandCatalog";
import type {
  GlobalProjectPort,
  NewProjectPort,
  OpenProjectOutcome,
  ProjectFailureDialogPort,
  RecentProjectSummary,
  RecentProjectFirstSheet,
} from "./application/globalProjectPort";
import { EditorUnavailableNotice } from "./EditorUnavailableNotice";
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
            width: `${Math.round(104 * sheet.widthUm * (sheet.activeSides === "both" ? 1 : 2) / sheet.heightUm)}px`,
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

function RecentProjectCard({
  project, now, disabled, favoriteDisabled, load, onOpen, onFavorite,
}: {
  project: RecentProjectSummary;
  now: Date;
  disabled: boolean;
  favoriteDisabled: boolean;
  load(id: string): Promise<RecentProjectFirstSheet | null>;
  onOpen(id: string): void;
  onFavorite(id: string, favorite: boolean): void;
}) {
  const name = useRef<HTMLElement>(null);
  const nameTooltipElement = useRef<HTMLDivElement>(null);
  const favoriteTrigger = useRef<HTMLButtonElement>(null);
  const favoriteLabel = project.favorite ? "Remover dos favoritos" : "Adicionar aos favoritos";
  const favoriteTooltip = useTooltipTriggerState({ delay: 600, closeDelay: 100 });
  const { triggerProps: favoriteTriggerProps, tooltipProps: favoriteDescriptionProps } =
    useTooltipTrigger({ delay: 600, closeDelay: 100 }, favoriteTooltip, favoriteTrigger);
  const { tooltipProps: favoriteTooltipProps } = useTooltip(favoriteDescriptionProps, favoriteTooltip);
  const [nameTooltipPosition, setNameTooltipPosition] = useState<{
    top: number;
    left: number;
    placement: "above" | "below";
  } | null>(null);
  const openedAt = recentProjectOpeningTime(project.lastOpenedAtMs, now);
  const nameTooltip = useTooltipTriggerState({ delay: 600, closeDelay: 100 });
  const closeNameTooltip = useRef(nameTooltip.close);
  closeNameTooltip.current = nameTooltip.close;
  const { tooltipProps: nameTooltipProps } = useTooltip({}, nameTooltip);
  useEffect(() => {
    if (!nameTooltip.isOpen) return;
    const dismissOnEscape = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        closeNameTooltip.current(true);
      }
    };
    document.addEventListener("keydown", dismissOnEscape, true);
    return () => document.removeEventListener("keydown", dismissOnEscape, true);
  }, [nameTooltip.isOpen]);

  useLayoutEffect(() => {
    if (!nameTooltip.isOpen) return;
    const anchor = name.current;
    const tooltip = nameTooltipElement.current;
    if (!anchor || !tooltip) return;
    const position = () => {
      const anchorRect = anchor.getBoundingClientRect();
      const tooltipRect = tooltip.getBoundingClientRect();
      const above = anchorRect.top - tooltipRect.height - 8;
      const placement = above >= 8 ? "above" : "below";
      const desiredTop = placement === "above" ? above : anchorRect.bottom + 8;
      setNameTooltipPosition({
        top: Math.max(8, Math.min(desiredTop, window.innerHeight - tooltipRect.height - 8)),
        left: Math.max(8, Math.min(anchorRect.left, window.innerWidth - tooltipRect.width - 8)),
        placement,
      });
    };
    const dismissOnScroll = () => closeNameTooltip.current(true);
    position();
    window.addEventListener("resize", position);
    window.addEventListener("scroll", dismissOnScroll, true);
    return () => {
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", dismissOnScroll, true);
    };
  }, [nameTooltip.isOpen]);

  const { hoverProps: nameHoverProps } = useHover({
    isDisabled: disabled,
    onHoverStart: () => {
      if (name.current && name.current.scrollWidth > name.current.clientWidth + 1) {
        nameTooltip.open();
      }
    },
    onHoverEnd: () => nameTooltip.close(true),
  });

  return (
    <li className="global-project-card" data-project-id={project.id}>
      <button
        aria-label={project.name}
        aria-description={openedAt ? `Última abertura: ${openedAt.fullLabel}` : undefined}
        disabled={disabled}
        onClick={() => onOpen(project.id)}
        type="button"
        className="global-project-launch"
      >
        <RecentProjectThumbnail id={project.id} load={load} />
        <span className="global-project-summary">
          <strong {...nameHoverProps} ref={name}>{project.name}</strong>
          {openedAt && (
            <time className="global-project-when" dateTime={openedAt.dateTime}>
              {openedAt.label}
            </time>
          )}
        </span>
      </button>
      <button
        {...favoriteTriggerProps}
        aria-label={favoriteLabel}
        aria-pressed={project.favorite}
        className="global-project-favorite"
        disabled={disabled || favoriteDisabled}
        onClick={() => onFavorite(project.id, !project.favorite)}
        ref={favoriteTrigger}
        type="button"
      >
        <AppIcon icon={Star} size={16} />
      </button>
      {favoriteTooltip.isOpen && (
        <div {...favoriteTooltipProps} className="ui-anchored-tooltip global-project-favorite-tooltip">
          {favoriteLabel}
        </div>
      )}
      {nameTooltip.isOpen && createPortal(
        <div
          {...nameTooltipProps}
          className="ui-anchored-tooltip global-project-name-tooltip"
          data-placement={nameTooltipPosition?.placement}
          ref={nameTooltipElement}
          style={{
            top: nameTooltipPosition?.top ?? 0,
            left: nameTooltipPosition?.left ?? 0,
            visibility: nameTooltipPosition ? "visible" : "hidden",
          }}
        >
          {project.name}
        </div>,
        document.body,
      )}
    </li>
  );
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
  const [favoritePending, setFavoritePending] = useState<string | null>(null);
  const restoreFavoriteFocus = useRef<string | null>(null);
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

  const setFavorite = async (id: string, favorite: boolean) => {
    if (favoritePending) return;
    setFavoritePending(id);
    const outcome = await projectPort.setRecentProjectFavorite(id, favorite);
    if (outcome.status === "saved") {
      restoreFavoriteFocus.current = outcome.projects.some((project) => project.id === id)
        ? id
        : (outcome.projects.find((project) => !project.favorite) ?? outcome.projects[0])?.id ?? "";
      setRecentProjects(outcome.projects);
    } else {
      await failureDialogPort.present({ context: "favoriteUpdate", error: outcome.error });
      restoreFavoriteFocus.current = id;
    }
    setFavoritePending(null);
  };

  useLayoutEffect(() => {
    if (favoritePending !== null) return;
    const id = restoreFavoriteFocus.current;
    if (id === null) return;
    restoreFavoriteFocus.current = null;
    const card = Array.from(document.querySelectorAll<HTMLElement>(".global-project-card"))
      .find((element) => element.dataset.projectId === id);
    const target = card?.querySelector<HTMLButtonElement>(".global-project-favorite")
      ?? newProjectTriggerRef.current;
    target?.focus({ preventScroll: true });
  }, [favoritePending, recentProjects]);

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

  // Without the editor, Welcome stays in place with its Project actions off and
  // the reason where the recent projects would be; settings keep working.
  const editorUnavailable = !graphicsDiagnostic.supported;

  if (surface === "newProject" && !editorUnavailable) {
    return (
      <div className="global-shell global-shell--new-project ui-chrome-selection-scope">
        <ApplicationHeader context="Novo projeto" showBrand={false} />
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
  const favorites = recentProjects.filter((project) => project.favorite);
  const nonFavorites = recentProjects.filter((project) => !project.favorite);
  const renderProjects = (projects: readonly RecentProjectSummary[]) => projects.map((project) => (
    <RecentProjectCard
      key={project.id}
      project={project}
      now={recentNow}
      disabled={isOpening}
      favoriteDisabled={favoritePending !== null}
      load={projectPort.firstRecentProjectSheet}
      onOpen={openRecentProject}
      onFavorite={setFavorite}
    />
  ));

  return (
    <div className="global-shell ui-chrome-selection-scope">
      <ApplicationHeader showBrand={false} status={editorUnavailable ? "Modo seguro" : undefined} />

      <main className="global-recent-projects">
        {editorUnavailable ? <EditorUnavailableNotice diagnostic={graphicsDiagnostic} /> : <>
        {recentProjects.length === 0 ? (
          <EmptyState
            className="global-empty-state"
            description="Crie um projeto ou abra um arquivo .myalbuns."
            title="Nenhum projeto recente"
          />
        ) : null}
        {favorites.length > 0 && <section className="global-project-section">
          <h1 className="ui-section-heading">
            Favoritos
            <span aria-hidden="true" className="ui-section-heading__count">{favorites.length}</span>
          </h1>
          <ul aria-label="Favoritos" className="global-recent-list">{renderProjects(favorites)}</ul>
        </section>}
        {nonFavorites.length > 0 && <section className="global-project-section">
          <h1 className="ui-section-heading">
            Projetos recentes
            <span aria-hidden="true" className="ui-section-heading__count">{nonFavorites.length}</span>
          </h1>
          <ul aria-label="Projetos recentes" className="global-recent-list">{renderProjects(nonFavorites)}</ul>
        </section>}
        </>}
      </main>

      <aside aria-label="Ações principais" className="global-primary-actions">
        <BrandWordmark subtitle="diagramação de álbuns" />
        <div className="global-action-stack">
          <ActionButton
            aria-label="Novo projeto"
            aria-keyshortcuts={projectCommandShortcutAria("new-project")}
            disabled={isOpening || editorUnavailable}
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
            disabled={isOpening || editorUnavailable}
            onClick={openProject}
          >
            <AppIcon icon={FolderOpen} size={16} />
            <span>
              {isOpening ? "Abrindo projeto…" : "Abrir projeto…"}
            </span>
            <kbd>{projectCommandShortcutLabel("open-project")}</kbd>
          </ActionButton>
        </div>
        <div className="global-secondary-actions">
          <button type="button" disabled={isOpening || !onOpenSettings} onClick={() => {
            setSettingsError(null);
            void onOpenSettings?.().catch(() => setSettingsError("Não foi possível abrir Configurações. Tente novamente."));
          }}>
            <AppIcon icon={Settings} size={14} />
            <span>Configurações…</span>
          </button>
          {settingsError && <p role="alert">{settingsError}</p>}
          <button
            aria-label="Exportação em lote"
            disabled={isOpening || editorUnavailable || !onOpenBatch}
            onClick={() => { void onOpenBatch?.().catch(() => setSettingsError("Não foi possível abrir a exportação em lote.")); }}
            type="button"
          >
            <AppIcon icon={Download} size={14} />
            <span>Exportação em lote</span>
          </button>
        </div>
        <p className="global-version">Versão 0.1.0</p>
      </aside>
    </div>
  );
}
