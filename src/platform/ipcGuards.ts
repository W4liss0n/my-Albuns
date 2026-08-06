export function isIpcRecord(
  value: unknown,
): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

export function isIpcRevision(value: unknown): value is number {
  return Number.isSafeInteger(value) && Number(value) >= 0;
}
