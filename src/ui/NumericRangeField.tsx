import type { ReactNode, Ref } from "react";
import { FieldValidationAutoTooltip, FieldValidationTooltip, type FieldValidationTooltipModel } from "./FieldValidationTooltip";
import "./NumericRangeField.css";

/** Shared layout/validation; live editor gestures and form drafts keep their own lifecycle. */
export function NumericRangeField({ label, unit, numberInput, slider, help, validation, ref }: {
  label: string;
  unit: string;
  numberInput: ReactNode;
  slider: ReactNode;
  help?: { id: string; text: string };
  validation: FieldValidationTooltipModel;
  ref?: Ref<HTMLDivElement>;
}) {
  return <div className="ui-numeric-range-field" ref={ref}>
    <div className="ui-numeric-range-heading">
      <span>{label}</span>
      <div className="ui-numeric-range-number">
        {numberInput}
        <span aria-hidden="true">{unit}</span>
      </div>
      <FieldValidationAutoTooltip field="value" tooltip={validation} />
    </div>
    {slider}
    {help && <p id={help.id} className="ui-numeric-range-help">{help.text}</p>}
    <FieldValidationTooltip tooltip={validation} />
  </div>;
}
