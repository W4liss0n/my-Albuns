import type { LayoutCandidate, LayoutQueryResult } from "../../domain/project";

export type LayoutCycleDirection = "next" | "previous";

/**
 * Cycle order for the keyboard: custom favorites, custom, automatic favorites,
 * automatic. The sort is stable, so inside each group the Core listing order
 * stands (last applied first, then favorite order, then catalog or generator
 * order), which is the same order the Layout panel shows.
 */
export function layoutCycleOrder(query: LayoutQueryResult): number[] {
  const rank = (candidate: LayoutCandidate) =>
    (candidate.layout.origin === "custom" ? 0 : 2) + (candidate.favoriteId ? 0 : 1);
  return query.listing.candidates
    .map((candidate, index) => ({ candidate, index }))
    .filter(({ index }) => !query.candidateRequiresLock[index])
    .sort((first, second) => rank(first.candidate) - rank(second.candidate))
    .map(({ index }) => index);
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
