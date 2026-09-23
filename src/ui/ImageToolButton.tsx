import { useRef, type ButtonHTMLAttributes } from "react";
import type { LucideIcon } from "lucide-react";
import { ActionButton } from "./ActionButton";
import { AppIcon } from "./AppIcon";
import { useUiAnchoredTooltip } from "./UiAnchoredTooltip";
import "./ImageToolButton.css";

interface ImageToolButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "title"> {
  label: string;
  icon: LucideIcon;
  glyph?: "navigation" | "tool";
}

export function ImageToolButton({ label, icon, glyph = "tool", className, disabled, ...props }: ImageToolButtonProps) {
  const button = useRef<HTMLButtonElement>(null);
  const tooltip = useUiAnchoredTooltip(button, label, disabled);
  return <>
    <ActionButton {...props} {...tooltip.triggerProps} ref={button} density="compact" variant="quiet" disabled={disabled}
      className={["ui-image-tool", className].filter(Boolean).join(" ")}
      data-glyph={glyph} aria-label={label}>
      <AppIcon icon={icon} size={18} />
    </ActionButton>
    {tooltip.tooltip}
  </>;
}
