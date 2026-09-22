import type { LucideIcon } from "lucide-react";
import { Focusable, Tooltip, TooltipTrigger } from "react-aria-components";
import { ActionButton } from "./ActionButton";
import { AppIcon } from "./AppIcon";
import "./PropertyToggle.css";

/** An icon tool whose whole surface reflects the property's state. */
export function PropertyToggle({ label, icon, pressed, disabled, onToggle, className }: {
  label: string;
  icon: LucideIcon;
  pressed: boolean | "mixed";
  disabled: boolean;
  onToggle(): void;
  className?: string;
}) {
  return (
    <TooltipTrigger delay={600} closeDelay={100} isDisabled={disabled}>
      <Focusable>
        <ActionButton
          className={["ui-property-toggle", className].filter(Boolean).join(" ")}
          density="compact"
          variant="quiet"
          aria-label={label}
          aria-pressed={pressed}
          disabled={disabled}
          onClick={onToggle}
        >
          <AppIcon icon={icon} size={16} />
        </ActionButton>
      </Focusable>
      <Tooltip className="ui-anchored-tooltip ui-property-toggle-tooltip" placement="right" offset={8}>
        {label}
      </Tooltip>
    </TooltipTrigger>
  );
}
