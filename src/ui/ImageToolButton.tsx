import { useId, useLayoutEffect, useRef, useState, type ButtonHTMLAttributes } from "react";
import { createPortal } from "react-dom";
import type { LucideIcon } from "lucide-react";
import { ActionButton } from "./ActionButton";
import { AppIcon } from "./AppIcon";
import "./ImageToolButton.css";

interface ImageToolButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, "children" | "title"> {
  label: string;
  icon: LucideIcon;
}

export function ImageToolButton({ label, icon, className, disabled, ...props }: ImageToolButtonProps) {
  const id = useId();
  const button = useRef<HTMLButtonElement>(null);
  const tooltipElement = useRef<HTMLSpanElement>(null);
  const [tooltip, setTooltip] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    if (!tooltip || !tooltipElement.current) return;
    const width = tooltipElement.current.getBoundingClientRect().width;
    tooltipElement.current.style.left = `${Math.max(width / 2 + 12, Math.min(window.innerWidth - width / 2 - 12, tooltip.left))}px`;
  }, [tooltip]);
  const show = () => {
    if (disabled || !button.current) return;
    const rect = button.current.getBoundingClientRect();
    setTooltip({ left: Math.max(12, Math.min(window.innerWidth - 12, rect.left + rect.width / 2)), top: rect.bottom + 42 > window.innerHeight ? rect.top - 42 : rect.bottom + 8 });
  };
  return <>
    <ActionButton {...props} ref={button} density="compact" variant="quiet" disabled={disabled}
      className={["ui-image-tool", className].filter(Boolean).join(" ")}
      aria-label={label} aria-describedby={tooltip ? id : undefined}
      onPointerEnter={show} onPointerLeave={() => setTooltip(null)}
      onFocus={show} onBlur={() => setTooltip(null)}>
      <AppIcon icon={icon} size={18} />
    </ActionButton>
    {tooltip && createPortal(<span id={id} ref={tooltipElement} role="tooltip" className="ui-anchored-tooltip ui-image-tool__tooltip"
      style={{ left: tooltip.left, top: tooltip.top }}>{label}</span>, document.body)}
  </>;
}
