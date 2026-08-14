import type { ReactNode } from "react";

interface DialogWindowFrameProps {
  actions?: ReactNode;
  children: ReactNode;
  layout: "message" | "progress";
  title: string;
}

export function DialogWindowFrame({
  actions,
  children,
  layout,
  title,
}: DialogWindowFrameProps) {
  return (
    <section
      aria-label={title}
      aria-modal="true"
      className={`ui-dialog-window ui-dialog-window--${layout}`}
      role="dialog"
    >
      {layout !== "message" ? (
        <header className="ui-dialog-window__header">
          <h2>{title}</h2>
        </header>
      ) : null}
      <div className="ui-dialog-window__body">{children}</div>
      {actions ? (
        <footer className="ui-dialog-window__footer">{actions}</footer>
      ) : null}
    </section>
  );
}
