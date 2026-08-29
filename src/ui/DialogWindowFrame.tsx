import { useId, type ReactNode } from "react";

type DialogWindowFrameProps = {
  actions?: ReactNode;
  children: ReactNode;
  title: string;
} & (
  | { layout: "message"; titleId: string }
  | { layout: "progress"; titleId?: never }
);

export function DialogWindowFrame({
  actions,
  children,
  layout,
  title,
  titleId,
}: DialogWindowFrameProps) {
  const generatedTitleId = useId();
  const accessibleTitleId = titleId ?? generatedTitleId;

  return (
    <section
      aria-labelledby={accessibleTitleId}
      aria-modal="true"
      className={`ui-dialog-window ui-dialog-window--${layout}`}
      role="dialog"
    >
      {layout !== "message" ? (
        <header className="ui-dialog-window__header">
          <h2 id={accessibleTitleId}>{title}</h2>
        </header>
      ) : null}
      <div className="ui-dialog-window__body">{children}</div>
      {actions ? (
        <footer className="ui-dialog-window__footer">{actions}</footer>
      ) : null}
    </section>
  );
}
