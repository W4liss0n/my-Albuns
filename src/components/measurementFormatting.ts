import type { DocumentSnapshot } from "../domain/project";

const MICROMETERS_PER_DISPLAY_UNIT = {
  mm: 1_000,
  cm: 10_000,
  in: 25_400,
} as const;

export function micrometersToDisplayUnits(
  micrometers: number,
  unit: DocumentSnapshot["displayUnit"],
) {
  return micrometers / MICROMETERS_PER_DISPLAY_UNIT[unit];
}
