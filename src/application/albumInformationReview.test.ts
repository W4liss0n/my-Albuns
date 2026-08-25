import { expect, test } from "vitest";

import type { AlbumInformation } from "../domain/project";
import {
  albumInformationReviewEquals,
  albumInformationReviewHasChanges,
  createAlbumInformationReview,
} from "./albumInformationReview";

const baseline: AlbumInformation = {
  displayUnit: "mm",
  sheetWidthUm: 600_000,
  sheetHeightUm: 300_000,
  dpi: 300,
  bleedUm: 3_000,
  safetyUm: 3_000,
  firstSheet: "double",
  lastSheet: "double",
};

const impact = {
  sheetWidthPx: 14_173,
  pageWidthPx: 7_087,
  heightPx: 7_087,
};

test("compares only facts visible in the Album Information confirmation", () => {
  const confirmed = createAlbumInformationReview(
    baseline,
    { ...baseline, dpi: 600 },
    impact,
  );
  const afterUnrelatedHistory = createAlbumInformationReview(
    { ...baseline, bleedUm: 5_000, safetyUm: 7_000 },
    { ...baseline, dpi: 600, bleedUm: 5_000, safetyUm: 7_000 },
    impact,
  );
  const afterRelevantHistory = createAlbumInformationReview(
    { ...baseline, dpi: 400 },
    { ...baseline, dpi: 600 },
    impact,
  );

  expect(albumInformationReviewEquals(confirmed, afterUnrelatedHistory)).toBe(
    true,
  );
  expect(albumInformationReviewEquals(confirmed, afterRelevantHistory)).toBe(
    false,
  );
});

test("recognizes when History already satisfied the confirmed intent", () => {
  expect(
    albumInformationReviewHasChanges(
      createAlbumInformationReview(baseline, baseline, impact),
    ),
  ).toBe(false);
});
