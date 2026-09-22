import { formatPhysicalMeasurement } from "../application/physicalMeasurements";
import type { DisplayUnit } from "../domain/project";

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
  return (
    <label className="ui-range-control">
      <span className="ui-range-control__heading">
        <span>{range.label}</span>
        <output>{kind === "border" && valueUm === 0 ? "sem borda" : formatPhysicalMeasurement(valueUm, displayUnit)}</output>
      </span>
      <input
        aria-label={label ?? range.accessibleLabel}
        className="ui-range"
        max={Math.max(range.max, includeValueUm ?? range.max)}
        min={0}
        step={range.step}
        type="range"
        value={valueUm}
        onChange={(event) => onChange(Number(event.currentTarget.value))}
      />
    </label>
  );
}
