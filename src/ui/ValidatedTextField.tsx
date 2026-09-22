import { forwardRef, useId, type ReactNode } from "react";
import { X } from "lucide-react";
import { AppIcon } from "./AppIcon";
import { TextInput } from "./TextInput";
import {
  FieldValidationAutoTooltip,
  fieldValidationTooltipAttributes,
  type FieldValidationTooltipModel,
} from "./FieldValidationTooltip";
import "./ValidatedTextField.css";

interface ValidatedTextFieldProps {
  controls?: ReactNode;
  density?: "regular" | "compact";
  disabled?: boolean;
  error?: string;
  field: string;
  hideLabel?: boolean;
  inputMode: "decimal" | "numeric";
  label: string;
  onChange(value: string): void;
  onReset?(): void;
  suffix?: string;
  validationTooltip: FieldValidationTooltipModel;
  value: string;
}

/** Field composition only; the form owns drafts, conversion and validation timing. */
export const ValidatedTextField = forwardRef<HTMLInputElement, ValidatedTextFieldProps>(
  function ValidatedTextField({
    controls, density = "regular", disabled, error, field, hideLabel,
    inputMode, label, onChange, onReset, suffix, validationTooltip, value,
  }, ref) {
    const inputId = useId();
    return (
      <div className={`ui-text-field ui-text-field--${density}`}>
        <label className={hideLabel ? "ui-visually-hidden" : undefined} htmlFor={inputId}>{label}</label>
        <span className={`ui-text-field__entry${suffix ? " ui-text-field__entry--suffix" : ""}${controls ? " ui-text-field__entry--controlled" : ""}`}>
          <TextInput
            aria-label={label}
            className={density === "compact" ? "ui-field-control" : undefined}
            disabled={disabled}
            id={inputId}
            inputMode={inputMode}
            onChange={(event) => onChange(event.currentTarget.value)}
            ref={ref}
            type="text"
            value={value}
            {...fieldValidationTooltipAttributes(field, error, validationTooltip)}
          />
          {onReset && <FieldResetButton label={label} onReset={onReset} />}
          {suffix && <span aria-hidden="true" className="ui-text-field__suffix">{suffix}</span>}
          {controls}
        </span>
        <FieldValidationAutoTooltip field={field} tooltip={validationTooltip} />
      </div>
    );
  },
);

export function FieldResetButton({ label, onReset }: { label: string; onReset(): void }) {
  return (
    <button
      aria-label={`Restaurar ${label}`}
      className="ui-field-reset"
      type="button"
      onClick={onReset}
      onPointerDown={(event) => event.preventDefault()}
    >
      <AppIcon icon={X} size={12} />
    </button>
  );
}
