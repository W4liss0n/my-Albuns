import type { LayoutQueryResult } from "../../domain/project";

export type LayoutCycleDirection = "next" | "previous";

/**
 * The Core lists the cycle order (custom favorites, custom, automatic
 * favorites, automatic) with the last applied Layout in its natural place.
 * Candidates that need the lock stay out of the keyboard cycle.
 */
export function layoutCycleOrder(query: LayoutQueryResult): number[] {
  return query.listing.cycleOrder.filter((index) => !query.candidateRequiresLock[index]);
}

/** Candidate index to apply, or null when there is nothing to change. */
export function nextLayoutIndex(query: LayoutQueryResult, direction: LayoutCycleDirection): number | null {
  if (query.locked) return null;
  const order = layoutCycleOrder(query);
  if (order.length === 0) return null;
  const current = order.findIndex((index) => query.listing.candidates[index].isLastApplied);
  if (current === -1) return direction === "next" ? order[0] : order[order.length - 1];
  if (order.length === 1) return null;
  const step = direction === "next" ? 1 : -1;
  return order[(current + step + order.length) % order.length];
}
