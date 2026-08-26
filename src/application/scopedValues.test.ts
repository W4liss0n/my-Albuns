import { expect, test } from "vitest";

import { mapScopedValue } from "./scopedValues";

test("maps a value shared by both sides without changing its scope", () => {
  expect(
    mapScopedValue(
      { scope: "bothSides", both: 3 },
      (value) => `item-${value}`,
    ),
  ).toEqual({ scope: "bothSides", both: "item-3" });
});

test("maps both independent sides without collapsing their scope", () => {
  expect(
    mapScopedValue(
      { scope: "perSide", left: 2, right: 5 },
      (value) => value * 10,
    ),
  ).toEqual({ scope: "perSide", left: 20, right: 50 });
});
