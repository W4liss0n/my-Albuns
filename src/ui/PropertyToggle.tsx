import type { LucideIcon } from "lucide-react";
import { ActionButton } from "./ActionButton";
import { AppIcon } from "./AppIcon";
import "./PropertyToggle.css";

/** A compact tool whose whole surface reflects the property's state. */
export function PropertyToggle({ label, icon, pressed, disabled, onToggle, className }: {
  label: string;
  icon: LucideIcon;
  pressed: boolean | "mixed";
  disabled: boolean;
  onToggle(): void;
  className?: string;
}) {
  return (
    <ActionButton
      className={["ui-property-toggle", className].filter(Boolean).join(" ")}
      density="compact"
      variant="quiet"
      aria-pressed={pressed}
      disabled={disabled}
      onClick={onToggle}
    >
      <AppIcon icon={icon} />
      <span>{label}</span>
    </ActionButton>
  );
}
