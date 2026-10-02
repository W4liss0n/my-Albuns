export function pixiColor(value: string): number {
  return Number.parseInt(value.replace("#", ""), 16);
}
