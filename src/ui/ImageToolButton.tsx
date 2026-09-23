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
  tooltipPlacement?: "top" | "bottom";
  tooltipText?: string;
  blocked?: boolean;
}

export function ImageToolButton({ label, icon, glyph = "tool", tooltipPlacement = "top", tooltipText, blocked = false, className, disabled, onClick, onKeyDown, ...props }: ImageToolButtonProps) {
  const button = useRef<HTMLButtonElement>(null);
  const tooltip = useUiAnchoredTooltip(button, tooltipText ?? label, disabled && !blocked, tooltipPlacement);
  return <>
    <ActionButton {...props} {...tooltip.triggerProps} ref={button} density="compact" variant="quiet" disabled={disabled && !blocked}
      aria-disabled={blocked || undefined}
      onClick={(event) => { tooltip.triggerProps.onClick?.(event); if (blocked) { event.preventDefault(); return; } onClick?.(event); }}
      onKeyDown={(event) => { tooltip.triggerProps.onKeyDown?.(event); if (blocked && (event.key === "Enter" || event.key === " ")) { event.preventDefault(); return; } onKeyDown?.(event); }}
      className={["ui-image-tool", className].filter(Boolean).join(" ")}
      data-glyph={glyph} aria-label={label}>
      <AppIcon icon={icon} size={18} />
    </ActionButton>
    {tooltip.tooltip}
  </>;
}
