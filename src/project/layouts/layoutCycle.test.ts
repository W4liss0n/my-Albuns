import { expect, test } from "vitest";
import type { LayoutCandidate, LayoutQueryResult } from "../../domain/project";
import { layoutCycleOrder, nextLayoutIndex } from "./layoutCycle";

function candidate(origin: "automatic" | "custom", options: { favorite?: boolean; last?: boolean } = {}): LayoutCandidate {
  return {
    isLastApplied: options.last ?? false,
    customId: origin === "custom" ? "custom-id" : null,
    favoriteId: options.favorite ? "favorite-id" : null,
    layout: { origin, definition: { surface: { type: "doubleSheet", widthUm: 600000, heightUm: 300000 }, scope: "page", positions: [] } },
  };
}

function query(candidates: LayoutCandidate[], cycleOrder: number[],
  options: { locked?: boolean; requiresLock?: boolean[] } = {}): LayoutQueryResult {
  return { queryId: "query", projectId: "project", revision: 1, catalogRevision: 0, sheetId: "sheet",
    frameCount: 2, locked: options.locked ?? false,
    candidateRequiresLock: options.requiresLock ?? candidates.map(() => false),
    settings: { permission: "pagesAndSheet", marginUm: 15000, gapUm: 5000, minimumSideUm: 20000 },
    listing: { algorithmVersion: 1, generationStatus: "candidates", candidates, cycleOrder } };
}

// The Core lists the last applied first and reports the cycle order with it in
// its natural place: here the last applied automatic sits between the two
// other automatic suggestions.
const listing = query([
  candidate("automatic", { last: true }),
  candidate("custom", { favorite: true }),
  candidate("automatic", { favorite: true }),
  candidate("custom"),
  candidate("automatic"),
  candidate("automatic"),
], [1, 3, 2, 4, 0, 5]);

test("steps through the Core cycle order from the last applied Layout", () => {
  expect(layoutCycleOrder(listing)).toEqual([1, 3, 2, 4, 0, 5]);
  expect(nextLayoutIndex(listing, "next")).toBe(5);
  expect(nextLayoutIndex(listing, "previous")).toBe(4);
});

test("without a last applied Layout, next starts at the first and previous at the last", () => {
  const fresh = query([candidate("automatic"), candidate("custom", { favorite: true }), candidate("automatic", { favorite: true })], [1, 2, 0]);
  expect(nextLayoutIndex(fresh, "next")).toBe(1);
  expect(nextLayoutIndex(fresh, "previous")).toBe(0);
});

test("the cycle wraps around at both ends", () => {
  const atEnd = query([candidate("automatic", { last: true }), candidate("custom"), candidate("automatic", { favorite: true })], [1, 2, 0]);
  expect(nextLayoutIndex(atEnd, "next")).toBe(1);
  const atStart = query([candidate("custom", { last: true, favorite: true }), candidate("custom"), candidate("automatic")], [0, 1, 2]);
  expect(nextLayoutIndex(atStart, "previous")).toBe(2);
});

test("candidates that require the lock stay out of the cycle", () => {
  const locked = query([candidate("automatic", { last: true }), candidate("custom"), candidate("automatic")], [1, 0, 2],
    { requiresLock: [false, true, false] });
  expect(layoutCycleOrder(locked)).toEqual([0, 2]);
  expect(nextLayoutIndex(locked, "next")).toBe(2);
});

test("a locked Sheet, an empty listing or a single candidate change nothing", () => {
  expect(nextLayoutIndex(query([candidate("custom"), candidate("automatic")], [0, 1], { locked: true }), "next")).toBeNull();
  expect(nextLayoutIndex(query([], []), "next")).toBeNull();
  expect(nextLayoutIndex(query([candidate("automatic", { last: true })], [0]), "next")).toBeNull();
  expect(nextLayoutIndex(query([candidate("automatic", { last: true })], [0]), "previous")).toBeNull();
});
