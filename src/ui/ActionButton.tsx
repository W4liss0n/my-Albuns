import type { ButtonHTMLAttributes } from "react";

export type ActionButtonVariant =
  | "primary"
  | "secondary"
  | "quiet"
  | "danger";

interface ActionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  density?: "compact" | "regular";
  variant?: ActionButtonVariant;
}

export function ActionButton({
  className,
  density = "regular",
  type = "button",
  variant = "secondary",
  ...props
}: ActionButtonProps) {
  return (
    <button
      {...props}
      className={[
        "ui-action-button",
        `ui-action-button--${variant}`,
        `ui-action-button--${density}`,
        className,
      ]
        .filter(Boolean)
        .join(" ")}
      type={type}
    />
  );
}
