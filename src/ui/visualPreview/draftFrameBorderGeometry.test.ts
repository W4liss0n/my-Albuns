import { expect, test } from "vitest";

import { draftFrameBorderFillRects } from "./draftFrameBorderGeometry";

test("keeps an oversized shared-preview Frame border fully inside", () => {
  expect(
    draftFrameBorderFillRects(
      { x: 10, y: 20, width: 100, height: 60 },
      100,
    ),
  ).toEqual([
    { x: 10, y: 20, width: 100, height: 60 },
    { x: 10, y: 20, width: 100, height: 60 },
    { x: 10, y: 20, width: 60, height: 60 },
    { x: 50, y: 20, width: 60, height: 60 },
  ]);
});
