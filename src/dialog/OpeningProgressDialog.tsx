import { useEffect, useRef, useState, type CSSProperties } from "react";

import type { OpeningProgress } from "../contracts/generated/OpeningProgress";
import type { OpeningProjectProgress } from "../contracts/generated/OpeningProjectProgress";
import { DialogWindowFrame } from "../ui/DialogWindowFrame";
import { ProgressBar, ProgressDialog } from "../ui";
import "./OpeningProgressDialog.css";

/** Width of the window while it lists several Projects. */
export const OPENING_PROJECT_LIST_WIDTH = 480;

interface OpeningImageProgress {
  completedFiles: number;
  totalFiles: number;
}

export type OpeningProgressView =
  | { kind: "single"; images: OpeningImageProgress }
  | { kind: "list"; projects: readonly OpeningProjectProgress[] };

/**
 * What the opening window shows for the latest rows. One Project keeps the
 * single-Project dialog, which switches to its photos once they are counted;
 * several Projects are listed. A decision page stays while its decision is
 * pending.
 */
export function openingProgressView(
  progress: OpeningProgress | null,
  decisionPage: boolean,
): OpeningProgressView | null {
  if (!progress) return null;
  const { projects } = progress;
  if (decisionPage && projects.some((project) => project.state === "deciding")) {
    return null;
  }
  if (projects.length > 1) return { kind: "list", projects };
  const [project] = projects;
  return project && project.totalFiles > 0 && project.state !== "deciding"
    ? { kind: "single", images: project }
    : null;
}

export function OpeningProgressDialog({
  creating = false,
  images = null,
}: {
  creating?: boolean;
  images?: OpeningImageProgress | null;
}) {
  return <ProgressDialog
    title={creating ? "Criando projeto" : "Abrindo projeto"}
    reserveProgressMeta
    progress={images ? {
      kind: "determinate",
      completed: images.completedFiles,
      total: images.totalFiles,
      status: "Preparando imagens",
    } : {
      kind: "indeterminate",
      status: "Preparando a Janela do projeto…",
    }}
  />;
}

function photoCount(project: OpeningProjectProgress) {
  const total = Math.max(0, project.totalFiles);
  const completed = Math.min(Math.max(0, project.completedFiles), total);
  return `${completed} de ${total} ${total === 1 ? "foto" : "fotos"}`;
}

function projectStatus(project: OpeningProjectProgress) {
  switch (project.state) {
    case "starting":
      return "Abrindo…";
    case "preparing":
      return photoCount(project);
    case "deciding":
      return "Aguardando sua escolha";
    case "ready":
      return "Pronto";
    case "failed":
      return "Não abriu";
    case "cancelled":
      return "Cancelado";
  }
}

function ProjectRow({ project }: { project: OpeningProjectProgress }) {
  const preparing = project.state === "preparing" && project.totalFiles > 0;
  const total = Math.max(1, project.totalFiles);
  const completed = Math.min(Math.max(0, project.completedFiles), total);
  return (
    <li className="opening-project-list__row">
      <span className="opening-project-list__name" title={project.name}>
        {project.name}
      </span>
      <span
        className="opening-project-list__status"
        data-state={project.state}
      >
        {projectStatus(project)}
      </span>
      <span className="opening-project-list__bar">
        {preparing ? (
          <ProgressBar
            completed={completed}
            indicatorStyle={{
              "--ui-progress-width": `${Math.round((completed / total) * 100)}%`,
            } as CSSProperties}
            title={project.name}
            total={total}
          />
        ) : null}
      </span>
    </li>
  );
}

const finalStates: ReadonlySet<OpeningProjectProgress["state"]> = new Set([
  "ready",
  "failed",
  "cancelled",
]);

/**
 * What a screen reader hears when Projects finish: each Project that has
 * just become ready, failed or cancelled. Photo counts are not announced.
 */
function useFinishedProjectsAnnouncement(
  projects: readonly OpeningProjectProgress[],
) {
  const previous = useRef(projects);
  const [announcement, setAnnouncement] = useState("");
  useEffect(() => {
    const before = previous.current;
    previous.current = projects;
    const finished = projects.filter((project, index) =>
      finalStates.has(project.state) && before[index]?.state !== project.state
    );
    if (finished.length > 0) {
      setAnnouncement(finished
        .map((project) => `${project.name}: ${projectStatus(project)}.`)
        .join(" "));
    }
  }, [projects]);
  return announcement;
}

/** Several Projects opened together: one row each, in opening order. */
export function OpeningProjectsDialog({
  projects,
}: {
  projects: readonly OpeningProjectProgress[];
}) {
  const announcement = useFinishedProjectsAnnouncement(projects);
  return (
    <DialogWindowFrame
      layout="progress"
      title={`Abrindo ${projects.length} projetos`}
    >
      <ul aria-label="Projetos" className="opening-project-list">
        {projects.map((project, index) => (
          <ProjectRow key={index} project={project} />
        ))}
      </ul>
      <p aria-live="polite" className="ui-visually-hidden" role="status">
        {announcement}
      </p>
    </DialogWindowFrame>
  );
}
