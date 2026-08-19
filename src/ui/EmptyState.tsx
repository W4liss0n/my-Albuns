import type { ReactNode } from "react";

interface EmptyStateProps {
  className?: string;
  density?: "compact" | "regular";
  description?: ReactNode;
  eyebrow?: ReactNode;
  icon?: ReactNode;
  title: string;
}

export function EmptyState({
  className,
  density = "regular",
  description,
  eyebrow,
  icon,
  title,
}: EmptyStateProps) {
  return (
    <div
      aria-label={title}
      className={["ui-empty-state", className].filter(Boolean).join(" ")}
      data-density={density}
      role="status"
    >
      {icon && <span className="ui-empty-state__icon">{icon}</span>}
      {eyebrow && (
        <span className="ui-empty-state__eyebrow">{eyebrow}</span>
      )}
      <strong>{title}</strong>
      {description && <p>{description}</p>}
    </div>
  );
}
