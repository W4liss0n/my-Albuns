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

function query(candidates: LayoutCandidate[], options: { locked?: boolean; requiresLock?: boolean[] } = {}): LayoutQueryResult {
  return { queryId: "query", projectId: "project", revision: 1, catalogRevision: 0, sheetId: "sheet",
    frameCount: 2, locked: options.locked ?? false,
    candidateRequiresLock: options.requiresLock ?? candidates.map(() => false),
    settings: { permission: "pagesAndSheet", marginUm: 15000, gapUm: 5000, minimumSideUm: 20000 },
    listing: { algorithmVersion: 1, generationStatus: "candidates", candidates } };
}

// The Core lists: last applied, favorites (any origin), custom, automatic.
const coreOrder = query([
  candidate("automatic", { last: true, favorite: true }),
  candidate("custom", { favorite: true }),
  candidate("automatic", { favorite: true }),
  candidate("custom"),
  candidate("automatic"),
]);

test("cycles custom favorites, custom, automatic favorites and automatic, keeping the Core order inside each group", () => {
  expect(layoutCycleOrder(coreOrder)).toEqual([1, 3, 0, 2, 4]);
  expect(nextLayoutIndex(coreOrder, "next")).toBe(2);
  expect(nextLayoutIndex(coreOrder, "previous")).toBe(3);
});

test("without a last applied Layout, next starts at the first and previous at the last", () => {
  const listing = query([candidate("automatic"), candidate("custom", { favorite: true }), candidate("automatic", { favorite: true })]);
  expect(nextLayoutIndex(listing, "next")).toBe(1);
  expect(nextLayoutIndex(listing, "previous")).toBe(0);
});

test("the cycle wraps around at both ends", () => {
  const atEnd = query([candidate("automatic", { last: true }), candidate("custom"), candidate("automatic", { favorite: true })]);
  expect(nextLayoutIndex(atEnd, "next")).toBe(1);
  const atStart = query([candidate("custom", { last: true, favorite: true }), candidate("custom"), candidate("automatic")]);
  expect(nextLayoutIndex(atStart, "previous")).toBe(2);
});

test("candidates that require the lock stay out of the cycle", () => {
  const listing = query([candidate("automatic", { last: true }), candidate("custom"), candidate("automatic")],
    { requiresLock: [false, true, false] });
  expect(layoutCycleOrder(listing)).toEqual([0, 2]);
  expect(nextLayoutIndex(listing, "next")).toBe(2);
});

test("a locked Sheet, an empty listing or a single candidate change nothing", () => {
  expect(nextLayoutIndex(query([candidate("custom"), candidate("automatic")], { locked: true }), "next")).toBeNull();
  expect(nextLayoutIndex(query([]), "next")).toBeNull();
  expect(nextLayoutIndex(query([candidate("automatic", { last: true })]), "next")).toBeNull();
  expect(nextLayoutIndex(query([candidate("automatic", { last: true })]), "previous")).toBeNull();
});
