import type { ReactNode } from "react";

/** A settings group: an edge-to-edge header band with an optional action, then its content. */
export function SettingsSection({ title, action, busy, className, children }: {
  title: string;
  action?: ReactNode;
  busy: boolean;
  className?: string;
  children: ReactNode;
}) {
  return <section aria-label={title} aria-busy={busy}
    className={["application-settings-panel", className].filter(Boolean).join(" ")}>
    <header className="application-settings-section-header">
      <h2>{title}</h2>
      {action}
    </header>
    <div className="application-settings-section-body">{children}</div>
  </section>;
}
