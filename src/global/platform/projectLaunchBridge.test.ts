import { expect, test } from "vitest";

import { parseProjectFailureDetails } from "./projectLaunchBridge";

test("the failure page reads only well-formed Projects", () => {
  expect(parseProjectFailureDetails(null)).toBeNull();
  expect(parseProjectFailureDetails([])).toBeNull();
  expect(parseProjectFailureDetails([
    { name: "A", message: "Não abriu.", action: "Abra de novo." },
    { name: "B" },
    { name: "C", message: "Não abriu.", action: 3 },
  ])).toEqual([
    { name: "A", message: "Não abriu.", action: "Abra de novo." },
    { name: "C", message: "Não abriu." },
  ]);
});
