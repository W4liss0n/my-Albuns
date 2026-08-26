import type { ButtonHTMLAttributes } from "react";

type ActionButtonVariant =
  | "primary"
  | "secondary"
  | "quiet";

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
