import type { ProjectFailureDetail } from "../global/application/globalProjectPort";
import "./ProjectFailureList.css";

/**
 * Projects opened together that did not open: each one by name, with its
 * own reason and the action that is still available.
 */
export function ProjectFailureList({
  projects,
}: {
  projects: readonly ProjectFailureDetail[];
}) {
  return (
    <ul aria-label="Projetos que não abriram" className="project-failure-list">
      {projects.map((project, index) => (
        <li className="project-failure-list__item" key={index}>
          {project.name ? (
            <p className="project-failure-list__name">{project.name}</p>
          ) : null}
          <p>{project.message}</p>
          {project.action ? <p>{project.action}</p> : null}
        </li>
      ))}
    </ul>
  );
}
