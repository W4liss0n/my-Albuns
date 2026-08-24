// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { expect, test } from "vitest";

function stylesheet(relativePath: string) {
  return readFileSync(new URL(relativePath, import.meta.url), "utf8");
}

test("keeps keyboard focus visible inside destructive actions", () => {
  const css = stylesheet("./ui.css");

  expect(css).toMatch(
    /\.ui-action-button--danger:focus-visible:not\(:disabled\)\s*\{[^}]*box-shadow:/s,
  );
  expect(css).toMatch(
    /\.ui-dialog-window__footer\s+\.ui-action-button--danger:focus-visible:not\(:disabled\)\s*\{[^}]*box-shadow:/s,
  );
});

test("keeps keyboard focus visible inside the active media folder chip", () => {
  const css = stylesheet("../components/MediaPanel.css");

  expect(css).toMatch(
    /\.media-folder-chip:focus-visible\s*\{[^}]*box-shadow:/s,
  );
  expect(css).toMatch(
    /\.media-folder-chip\.active:focus-visible\s*\{[^}]*box-shadow:/s,
  );
});

test("keeps keyboard focus distinct inside the selected Sheet design scope", () => {
  const css = stylesheet("../components/SheetDesignInspector.css");

  expect(css).toMatch(
    /\.sheet-design-preview__target:focus-visible\s*\{[^}]*box-shadow:\s*inset 0 0 0 1px var\(--ui-text-muted\);/s,
  );
});
