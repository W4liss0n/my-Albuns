export type VisualScope = "both" | "left" | "right";

export type ScopedValue<T> =
  | { scope: "bothSides"; both: T }
  | { scope: "perSide"; left: T; right: T };

export type ScopedValueRead<T> =
  | { kind: "uniform"; value: T }
  | { kind: "mixed"; left: T; right: T };

/**
 * Reads the value represented by a spatial scope without silently choosing one
 * side when both sides differ.
 */
export function readScopedValue<T>(
  scoped: ScopedValue<T>,
  scope: VisualScope,
  equals: (left: T, right: T) => boolean = Object.is,
): ScopedValueRead<T> {
  if (scoped.scope === "bothSides") {
    return { kind: "uniform", value: scoped.both };
  }
  if (scope === "left" || scope === "right") {
    return { kind: "uniform", value: scoped[scope] };
  }
  return equals(scoped.left, scoped.right)
    ? { kind: "uniform", value: scoped.left }
    : { kind: "mixed", left: scoped.left, right: scoped.right };
}

export function applyScopedValue<T>(
  scoped: ScopedValue<T>,
  scope: VisualScope,
  value: T,
): ScopedValue<T> {
  if (scope === "both") {
    return { scope: "bothSides", both: value };
  }
  const left = valueAtSide(scoped, "left");
  const right = valueAtSide(scoped, "right");
  return scope === "left"
    ? { scope: "perSide", left: value, right }
    : { scope: "perSide", left, right: value };
}

/**
 * Projects content while preserving whether it is shared by both sides or
 * independently owned by each side.
 */
export function mapScopedValue<T, U>(
  scoped: ScopedValue<T>,
  map: (value: T) => U,
): ScopedValue<U> {
  return scoped.scope === "bothSides"
    ? { scope: "bothSides", both: map(scoped.both) }
    : {
        scope: "perSide",
        left: map(scoped.left),
        right: map(scoped.right),
      };
}

function valueAtSide<T>(scoped: ScopedValue<T>, side: "left" | "right"): T {
  return scoped.scope === "bothSides" ? scoped.both : scoped[side];
}
