import type { LucideIcon } from "lucide-react";
import { useRef } from "react";
import { useTooltip, useTooltipTrigger } from "react-aria";
import { useTooltipTriggerState } from "react-stately";
import { ActionButton } from "./ActionButton";
import { AppIcon } from "./AppIcon";
import "./PropertyToggle.css";

/** An icon tool whose whole surface reflects the property's state. */
export function PropertyToggle({ label, icon, pressed, disabled, onToggle, className, tooltipSide = "right" }: {
  label: string;
  icon: LucideIcon;
  pressed: boolean | "mixed";
  disabled: boolean;
  onToggle(): void;
  className?: string;
  tooltipSide?: "left" | "right";
}) {
  const trigger = useRef<HTMLButtonElement>(null);
  const options = { isDisabled: disabled, delay: 600, closeDelay: 100 };
  const tooltip = useTooltipTriggerState(options);
  const { triggerProps, tooltipProps: descriptionProps } = useTooltipTrigger(options, tooltip, trigger);
  const { tooltipProps } = useTooltip(descriptionProps, tooltip);
  return (
    <div className="ui-property-toggle-anchor" data-tooltip-side={tooltipSide}>
      <ActionButton
        {...triggerProps}
        className={["ui-property-toggle", className].filter(Boolean).join(" ")}
        density="compact"
        variant="quiet"
        aria-label={label}
        aria-pressed={pressed}
        disabled={disabled}
        onClick={onToggle}
        ref={trigger}
      >
        <AppIcon icon={icon} size={16} />
      </ActionButton>
      {tooltip.isOpen && (
        <div {...tooltipProps} className="ui-anchored-tooltip ui-property-toggle-tooltip">
          {label}
        </div>
      )}
    </div>
  );
}
