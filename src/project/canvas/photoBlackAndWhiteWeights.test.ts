// @vitest-environment node
import { expect, test } from "vitest";

import {
  PHOTO_BLACK_AND_WHITE_SVG_MATRIX,
  PHOTO_BLACK_AND_WHITE_WEIGHTS,
} from "./photoBlackAndWhiteWeights";

test("uses the integer luminance weights of the renderer contract, summing to one in 8.8 fixed point", () => {
  // Design 0019: y = floor((54 * r + 183 * g + 19 * b + 128) / 256).
  expect(PHOTO_BLACK_AND_WHITE_WEIGHTS).toEqual([54, 183, 19]);
  expect(
    PHOTO_BLACK_AND_WHITE_WEIGHTS.reduce((sum, weight) => sum + weight, 0),
  ).toBe(256);
});

test("the SVG preview matrix writes the same luminance to every colour channel and keeps alpha", () => {
  const values = PHOTO_BLACK_AND_WHITE_SVG_MATRIX.split(" ").map(Number);
  expect(values).toHaveLength(20);

  const [red, green, blue] = PHOTO_BLACK_AND_WHITE_WEIGHTS;
  const luminanceRow = [red / 256, green / 256, blue / 256, 0, 0];
  expect(values.slice(0, 5)).toEqual(luminanceRow);
  expect(values.slice(5, 10)).toEqual(luminanceRow);
  expect(values.slice(10, 15)).toEqual(luminanceRow);
  expect(values.slice(15, 20)).toEqual([0, 0, 0, 1, 0]);
});
