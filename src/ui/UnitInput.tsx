import { forwardRef } from "react";
import { TextInput, type TextInputProps } from "./TextInput";
import "./IntegratedControl.css";
import "./UnitInput.css";

/** The suffix shares the field surface; only the number belongs to its value. */
export const UnitInput = forwardRef<HTMLInputElement, TextInputProps & { unit: string }>(
  function UnitInput({ unit, className, ...props }, ref) {
    return (
      <span className="ui-unit-input">
        <TextInput
          {...props}
          ref={ref}
          className={["ui-field-control", "ui-integrated-control", "ui-unit-input__field", className].filter(Boolean).join(" ")}
        />
        <span className="ui-unit-input__unit" aria-hidden="true">{unit}</span>
      </span>
    );
  },
);
