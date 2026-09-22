import { useId, useState } from "react";
import { createPhysicalFieldDraft, editPhysicalFieldDraft, displayUnitLabel, formatMicrometers, formatPhysicalMeasurement, type PhysicalFieldDraft } from "../application/physicalMeasurements";
import type { DisplayUnit } from "../domain/project";
import { TextInput } from "../ui/TextInput";
import { NumericRangeField } from "../ui/NumericRangeField";
import { fieldValidationTooltipAttributes, useFieldValidationTooltip } from "../ui/FieldValidationTooltip";

const RANGES = {
  border: { label: "Borda padrão", accessibleLabel: "Espessura da borda", max: 5_000, step: 250 },
  gap: { label: "Espaço entre quadros", accessibleLabel: "Espaço entre quadros", max: 24_000, step: 1_000 },
} as const;

interface FrameDefaultRangeControlProps {
  kind: keyof typeof RANGES;
  valueUm: number;
  displayUnit: DisplayUnit;
  /** Editing can retain values beyond the standard creation range. */
  includeValueUm?: number;
  label?: string;
  onChange(valueUm: number): void;
}

export function FrameDefaultRangeControl({
  kind, valueUm, displayUnit, includeValueUm, label, onChange,
}: FrameDefaultRangeControlProps) {
  const range = RANGES[kind];
  const maximum = Math.max(range.max, includeValueUm ?? range.max);
  const unit = displayUnitLabel(displayUnit);
  const accessibleLabel = label ?? range.accessibleLabel;
  const [editing, setEditing] = useState<{
    unit: DisplayUnit; field: PhysicalFieldDraft; initialUm: number; sentUm: number;
  } | null>(null);
  // External updates or unit changes supersede a local field draft.
  const draft = editing?.unit === displayUnit && editing.sentUm === valueUm ? editing : null;
  const field = draft?.field ?? createPhysicalFieldDraft(valueUm, displayUnit);
  const valid = field.hasExactValue && field.valueUm >= 0 && field.valueUm <= maximum;
  const error = valid ? undefined : `Use uma medida entre 0 e ${formatPhysicalMeasurement(maximum, displayUnit)}.`;
  const validation = useFieldValidationTooltip(useId(), [{ field: "value", messages: error ? [error] : undefined }]);
  const cancel = () => {
    if (draft && draft.initialUm !== valueUm) onChange(draft.initialUm);
    setEditing(null);
  };
  const edit = (text: string) => {
    const next = editPhysicalFieldDraft(field, text, displayUnit);
    const initialUm = draft?.initialUm ?? valueUm;
    const sentUm = next.hasExactValue && next.valueUm >= 0 && next.valueUm <= maximum ? next.valueUm : initialUm;
    setEditing({ unit: displayUnit, field: next, initialUm, sentUm });
    if (sentUm !== valueUm) onChange(sentUm);
  };
  return (
    <NumericRangeField label={range.label} unit={unit} validation={validation}
      numberInput={<TextInput className="ui-field-control" type="text" inputMode="decimal" role="spinbutton"
        aria-label={`${accessibleLabel} em ${unit}`} value={field.text}
        aria-valuemin={0} aria-valuemax={Number(formatMicrometers(maximum, displayUnit))}
        aria-valuenow={Number(formatMicrometers(valueUm, displayUnit))}
        {...fieldValidationTooltipAttributes("value", error, validation)}
        onMouseEnter={() => { if (error) validation.show("value"); }}
        onChange={(event) => edit(event.currentTarget.value)}
        onBlur={() => { if (valid) setEditing(null); else cancel(); }}
        onKeyDown={(event) => {
          if (event.key === "Escape" || event.key === "Enter") {
            event.preventDefault(); event.stopPropagation();
            if (event.key === "Escape" || !valid) cancel(); else setEditing(null);
          } else if (event.key === "ArrowUp" || event.key === "ArrowDown") {
            event.preventDefault(); event.stopPropagation();
            const next = Math.max(0, Math.min(maximum, valueUm + (event.key === "ArrowUp" ? range.step : -range.step)));
            setEditing({ unit: displayUnit, field: createPhysicalFieldDraft(next, displayUnit), initialUm: draft?.initialUm ?? valueUm, sentUm: next });
            onChange(next);
          }
        }} />}
      slider={<input
        aria-label={accessibleLabel}
        aria-valuetext={kind === "border" && valueUm === 0 ? "sem borda" : formatPhysicalMeasurement(valueUm, displayUnit)}
        className="ui-range ui-numeric-range-slider"
        max={maximum}
        min={0}
        step={range.step}
        type="range"
        value={valueUm}
        onChange={(event) => { setEditing(null); onChange(Number(event.currentTarget.value)); }}
      />}
    />
  );
}
