import { forwardRef, type ButtonHTMLAttributes } from "react";

type ActionButtonVariant =
  | "primary"
  | "secondary"
  | "quiet";

interface ActionButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  density?: "compact" | "regular";
  variant?: ActionButtonVariant;
}

export const ActionButton = forwardRef<HTMLButtonElement, ActionButtonProps>(
  function ActionButton(
    {
      className,
      density = "regular",
      type = "button",
      variant = "secondary",
      ...props
    },
    ref,
  ) {
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
        ref={ref}
        type={type}
      />
    );
  },
);
