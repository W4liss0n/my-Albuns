import { renderHook } from "@testing-library/react";
import { expect, test } from "vitest";

import { useSemanticBaseline } from "./useSemanticBaseline";

test("preserves an equivalent committed baseline and exposes a semantic change immediately", () => {
  const first = { width: 600 };
  const equivalent = { width: 600 };
  const changed = { width: 700 };
  const view = renderHook(
    ({ signature, value }) => useSemanticBaseline(value, signature),
    { initialProps: { signature: "600", value: first } },
  );

  view.rerender({ signature: "600", value: equivalent });
  expect(view.result.current).toBe(first);

  view.rerender({ signature: "700", value: changed });
  expect(view.result.current).toBe(changed);
});
