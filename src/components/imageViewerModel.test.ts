import { expect, test } from "vitest";
import { representativeProjection } from "../test/projectFixtures";
import { adjacentViewerDemand, sheetViewerMediaIds, viewerPreviewState } from "./imageViewerModel";

test("sheet sequence follows composed frame order and includes each photo once", () => {
  const sheet = structuredClone(representativeProjection.composition.sheets[0]);
  sheet.frames.push({ ...sheet.frames[0], frameId: "another-use" });
  sheet.frames.push({ ...sheet.frames[0], frameId: "next-photo", photo: { ...sheet.frames[0].photo!, mediaId: "media-002" } });
  expect(sheetViewerMediaIds(sheet)).toEqual(["media-001", "media-002"]);
});

test("viewer demand retains only current and adjacent images", () => {
  expect(adjacentViewerDemand(["a", "b", "c", "d"], "b")).toEqual({
    visibleMediaIds: ["b"], preloadMediaIds: ["a", "c"],
  });
  expect(adjacentViewerDemand(["a", "b"], "b")).toEqual({
    visibleMediaIds: ["b"], preloadMediaIds: ["a"],
  });
});

test("viewer distinguishes original absence from cache failure even when a stale preview is retained", () => {
  const original = (state: "available" | "absent" | "unavailable") => ({ mediaId: "a", state, createdAtMs: null, modifiedAtMs: null });
  const preview = (state: "ready" | "absent" | "unavailable" | "cache_unavailable" | "cache_paused") => ({ mediaId: "a", state, url: "myalbuns-cache://localhost/stale" });
  expect(viewerPreviewState(original("absent"), preview("ready"))).toBe("absent");
  expect(viewerPreviewState(original("unavailable"), preview("ready"))).toBe("unavailable");
  expect(viewerPreviewState(original("available"), preview("cache_unavailable"))).toBe("cache_unavailable");
  expect(viewerPreviewState(original("available"), preview("cache_paused"))).toBe("cache_paused");
  expect(viewerPreviewState(original("available"), preview("ready"))).toBe("ready");
  expect(viewerPreviewState(original("available"), undefined)).toBe("loading");
});
