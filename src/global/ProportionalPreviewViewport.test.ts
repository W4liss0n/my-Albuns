// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { expect, test } from "vitest";

const viewportStyles = readFileSync(
  "src/global/ProportionalPreviewViewport.css",
  "utf8",
) as string;

test("lets the proportional preview use all available panel space", () => {
  const viewportRule = viewportStyles.match(
    /\.new-project-proportional-preview-viewport\s*\{([^}]*)\}/s,
  )?.[1];

  expect(viewportRule).toBeDefined();
  expect(viewportRule).toMatch(/(?:^|\s)width:\s*100%;/);
  expect(viewportRule).toMatch(/(?:^|\s)height:\s*100%;/);
  expect(viewportRule).not.toMatch(/width:\s*min\(/);
  expect(viewportRule).not.toMatch(/max-width:\s*\d+px/);
});
