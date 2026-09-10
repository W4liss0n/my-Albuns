import type { LayoutExportProblem } from "../domain/project";

/** Shared by the export failure and owned-dialog transports. */
export function parseLayoutExportProblems(value: unknown): LayoutExportProblem[] | null {
  if (!Array.isArray(value)) return null;
  const problems: LayoutExportProblem[] = [];
  for (const item of value) {
    if (typeof item !== "object" || item === null ||
        typeof item.sheetId !== "string" || item.sheetId.length === 0 ||
        typeof item.frameId !== "string" || item.frameId.length === 0 ||
        !Number.isSafeInteger(item.sheetNumber) || item.sheetNumber < 1 ||
        !Number.isSafeInteger(item.frameNumber) || item.frameNumber < 1) return null;
    problems.push({ sheetId: item.sheetId, sheetNumber: item.sheetNumber,
      frameId: item.frameId, frameNumber: item.frameNumber });
  }
  return problems;
}
