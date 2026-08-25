import type { HTMLAttributes, ReactNode } from "react";

interface InlineNoticeProps extends HTMLAttributes<HTMLElement> {
  children: ReactNode;
  title?: string;
  tone?: "error" | "info" | "success" | "warning";
}

export function InlineNotice({
  children,
  className,
  title,
  tone = "info",
  ...props
}: InlineNoticeProps) {
  return (
    <section
      {...props}
      className={[
        "ui-inline-notice",
        `ui-inline-notice--${tone}`,
        className,
      ]
        .filter(Boolean)
        .join(" ")}
    >
      {title ? <h2>{title}</h2> : null}
      <div>{children}</div>
    </section>
  );
}
