import type { LucideIcon } from "lucide-react";
import { useRef } from "react";
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
  const trigger = useRef<HTMLButtonElement>(null);
  const tooltipOffset = 8;
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
          ref={trigger}
        >
          <AppIcon icon={icon} size={16} />
        </ActionButton>
      </Focusable>
      <Tooltip className="ui-anchored-tooltip ui-property-toggle-tooltip" placement="right" offset={tooltipOffset}
        style={({ placement }) => {
          const bounds = trigger.current?.getBoundingClientRect();
          if (!bounds) return {};
          // The portal uses document coordinates; client rects include root CSS zoom.
          const zoom = Number(getComputedStyle(document.documentElement).zoom) || 1;
          return {
            top: (bounds.top + bounds.height / 2) / zoom,
            left: placement === "left" ? "auto" : bounds.right / zoom + tooltipOffset,
            right: placement === "left" ? (window.innerWidth - bounds.left) / zoom + tooltipOffset : "auto",
            transform: "translateY(-50%)",
          };
        }}>
        {label}
      </Tooltip>
    </TooltipTrigger>
  );
}
