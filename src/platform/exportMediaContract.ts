import type { ExportMediaProblem } from "../application/exportMedia";

export function parseExportMediaProblems(value: unknown): ExportMediaProblem[] | null {
  if (!Array.isArray(value)) return null;
  const problems: ExportMediaProblem[] = [];
  for (const item of value) {
    if (typeof item !== "object" || item === null || typeof item.mediaId !== "string" ||
      typeof item.fileName !== "string" || (item.state !== "absent" && item.state !== "unavailable")) return null;
    problems.push({ mediaId: item.mediaId, fileName: item.fileName, state: item.state });
  }
  return problems;
}
