import { Check, Minus } from "lucide-react";
import { ActionButton } from "./ActionButton";
import "./PropertyToggle.css";

/** A compact property row with a persistent on/off/mixed indicator. */
export function PropertyToggle({ label, pressed, disabled, onToggle, className }: {
  label: string;
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
      <span>{label}</span>
      <span className="ui-property-toggle__indicator" aria-hidden="true">
        {pressed === "mixed" ? <Minus size={12} /> : pressed ? <Check size={12} /> : null}
      </span>
    </ActionButton>
  );
}
