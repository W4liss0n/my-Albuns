import type { DisplayUnit } from "../domain/project";

export interface PhysicalFieldDraft {
  text: string;
  valueUm: number;
  hasExactValue: boolean;
}

const MICROMETERS_PER_UNIT: Record<DisplayUnit, bigint> = {
  mm: 1_000n,
  cm: 10_000n,
  in: 25_400n,
};

const MAX_SAFE_INTEGER = BigInt(Number.MAX_SAFE_INTEGER);
const PRESENTATION_DECIMALS: Record<DisplayUnit, number> = {
  mm: 3,
  cm: 4,
  in: 3,
};

export function createPhysicalFieldDraft(
  valueUm: number,
  unit: DisplayUnit,
): PhysicalFieldDraft {
  return {
    text: formatMicrometers(valueUm, unit),
    valueUm,
    hasExactValue: true,
  };
}

export function editPhysicalFieldDraft(
  field: PhysicalFieldDraft,
  text: string,
  unit: DisplayUnit,
): PhysicalFieldDraft {
  const parsed = parsePhysicalText(text, unit);
  return {
    text,
    valueUm: parsed ?? field.valueUm,
    hasExactValue: parsed !== null,
  };
}

export function formatMicrometers(valueUm: number, unit: DisplayUnit): string {
  const negative = valueUm < 0;
  const magnitude = BigInt(Math.abs(valueUm));
  const denominator = MICROMETERS_PER_UNIT[unit];
  const decimalPlaces = PRESENTATION_DECIMALS[unit];
  const scale = 10n ** BigInt(decimalPlaces);
  const rounded = (magnitude * scale + denominator / 2n) / denominator;
  const integer = rounded / scale;
  const decimals = (rounded % scale)
    .toString()
    .padStart(decimalPlaces, "0")
    .replace(/0+$/, "");
  const sign = negative && rounded !== 0n ? "-" : "";

  return `${sign}${integer}${decimals ? `.${decimals}` : ""}`;
}

export function displayUnitLabel(unit: DisplayUnit): string {
  return unit === "in" ? "pol" : unit;
}

export function formatPhysicalMeasurement(
  valueUm: number,
  unit: DisplayUnit,
): string {
  return `${formatMicrometers(valueUm, unit)} ${displayUnitLabel(unit)}`;
}

export function parsePhysicalText(
  text: string,
  unit: DisplayUnit,
): number | null {
  const normalized = text.trim().replace(",", ".");
  if (!/^[+-]?(?:\d+(?:\.\d*)?|\.\d+)$/.test(normalized)) {
    return null;
  }

  const negative = normalized.startsWith("-");
  const unsigned = normalized.replace(/^[+-]/, "");
  const [whole = "0", fraction = ""] = unsigned.split(".");
  const denominator = 10n ** BigInt(fraction.length);
  const digits = `${whole || "0"}${fraction}`;
  let numerator = BigInt(digits || "0") * MICROMETERS_PER_UNIT[unit];
  if (negative) numerator = -numerator;
  if (numerator % denominator !== 0n) return null;

  const value = numerator / denominator;
  if (value > MAX_SAFE_INTEGER || value < -MAX_SAFE_INTEGER) return null;
  return Number(value);
}
