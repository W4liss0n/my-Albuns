import { expect, test } from "vitest";
import { representativeProjection } from "../test/projectFixtures";
import { adjacentViewerDemand, sheetViewerMediaIds } from "./imageViewerModel";

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
