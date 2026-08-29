import type { ReactNode } from "react";

import { ApplicationHeader } from "../ui";
import "./ProjectRecoveryOwnerShell.css";

export function ProjectRecoveryOwnerShell({
  children,
  modal,
  status,
}: {
  children?: ReactNode;
  modal: boolean;
  status: string;
}) {
  return (
    <main
      aria-busy={!modal || undefined}
      className="project-recovery-owner ui-chrome-selection-scope"
    >
      <div
        aria-hidden={modal ? "true" : undefined}
        className="project-recovery-owner__surface"
        data-project-owner-surface
        inert={modal ? true : undefined}
      >
        <ApplicationHeader context="Projeto" status={status} />
        <div aria-hidden="true" className="project-recovery-owner__commandbar">
          <span />
          <span />
          <span />
        </div>
        <div className="project-recovery-owner__workspace">
          <section className="project-recovery-owner__canvas">
            <span className="loading-mark" aria-hidden="true" />
          </section>
          <aside aria-hidden="true" className="project-recovery-owner__inspector" />
          <section aria-hidden="true" className="project-recovery-owner__media" />
        </div>
      </div>
      {children}
    </main>
  );
}
