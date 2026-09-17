export type { ExportFormat } from "../contracts/generated/ExportFormat";
export type { NormalExportOptions } from "../contracts/generated/NormalExportOptions";
import type { NormalExportOptions } from "../contracts/generated/NormalExportOptions";
export type { ExportSheetInfo } from "../contracts/generated/ExportSheetInfo";

export class ExportConflictsError extends Error {
  constructor(readonly files: string[]) { super("Já existem arquivos no destino da Exportação."); }
}

export function parseNormalExportOptions(value: unknown): NormalExportOptions | null {
  if (!value || typeof value !== "object" || !("sheetIds" in value) || !Array.isArray(value.sheetIds)
    || !("scope" in value) || (value.scope !== "album" && value.scope !== "range")
    || !value.sheetIds.length || !value.sheetIds.every(id => typeof id === "string" && id.length > 0)
    || new Set(value.sheetIds).size !== value.sheetIds.length || !("mode" in value) || !["sheet", "page"].includes(String(value.mode))
    || !("destination" in value) || typeof value.destination !== "string" || !("conflictPolicy" in value) || (value.conflictPolicy !== "ask" && value.conflictPolicy !== "skip" && value.conflictPolicy !== "replace")
    || !("format" in value) || !value.format || typeof value.format !== "object" || !("kind" in value.format)) return null;
  const format = value.format;
  if (format.kind === "jpeg") {
    if (!("quality" in format) || typeof format.quality !== "number" || !Number.isInteger(format.quality) || format.quality < 1 || format.quality > 100) return null;
    return { scope: value.scope, sheetIds: value.sheetIds, mode: value.mode as "sheet" | "page", destination: value.destination, conflictPolicy: value.conflictPolicy, format: { kind: "jpeg", quality: format.quality } };
  }
  if ((format.kind !== "png" && format.kind !== "pdf") || "quality" in format) return null;
  return { scope: value.scope, sheetIds: value.sheetIds, mode: value.mode as "sheet" | "page", destination: value.destination, conflictPolicy: value.conflictPolicy, format: { kind: format.kind } };
}
