import type { EditorProjection } from "../domain/project";

export function isIpcRecord(
  value: unknown,
): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function isIpcRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}

export function isIpcEditorProjection(
  value: unknown,
): value is EditorProjection {
  return (
    isIpcRecord(value) &&
    isIpcRecord(value.state) &&
    typeof value.state.projectId === "string" &&
    value.state.projectId.length > 0 &&
    isIpcRevision(value.state.revision) &&
    isIpcRevision(value.state.savedRevision)
  );
}

export function hasOnlyIpcKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
) {
  const actual = Object.keys(value).sort();
  const sortedExpected = [...expected].sort();
  return (
    actual.length === sortedExpected.length &&
    actual.every((key, index) => key === sortedExpected[index])
  );
}
