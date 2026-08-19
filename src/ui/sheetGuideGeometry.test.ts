import { expect, test } from "vitest";

import { createSheetGuideGeometry } from "./sheetGuideGeometry";

test("projects both technical guides directly from physical geometry", () => {
  expect(
    createSheetGuideGeometry({
      bleedUm: 3_000,
      heightUm: 300_000,
      safetyUm: 5_000,
    }),
  ).toEqual({
    bleedInsetUm: 3_000,
    dashGapUm: 2_400,
    dashLengthUm: 3_000,
    safetyInsetUm: 8_000,
    strokeWidthUm: 900,
  });

  expect(
    createSheetGuideGeometry({
      bleedUm: 100_000,
      heightUm: 300_000,
      safetyUm: 10_000,
    }),
  ).toMatchObject({
    bleedInsetUm: 100_000,
    safetyInsetUm: 110_000,
  });
});
