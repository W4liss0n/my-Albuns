import type { HTMLAttributes, ReactNode } from "react";

interface InlineNoticeProps extends HTMLAttributes<HTMLElement> {
  children: ReactNode;
  floating?: boolean;
  title?: string;
  tone?: "error" | "info" | "success" | "warning";
}

export function InlineNotice({
  children,
  className,
  floating = false,
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
        floating && "ui-inline-notice--floating",
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
