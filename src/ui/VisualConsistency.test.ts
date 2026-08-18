// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { expect, test } from "vitest";

const readStyles = (path: string) => readFileSync(path, "utf8") as string;

const themeStyles = readStyles("src/ui/theme.css");
const sharedStyles = readStyles("src/ui/ui.css");
const welcomeStyles = readStyles("src/global/GlobalShell.css");
const newProjectStyles = readStyles("src/global/NewProjectFlow.css");
const previewStyles = readStyles("src/global/NewProjectPreviewPanel.css");

test("centralizes the shared type scale used by welcome and new Project", () => {
  expect(themeStyles).toContain("--ui-font-size-micro: 9.5px;");
  expect(themeStyles).toContain("--ui-font-size-caption: 10.5px;");
  expect(themeStyles).toContain("--ui-font-size-support: 11px;");
  expect(themeStyles).toContain("--ui-font-size-label: 11.5px;");
  expect(themeStyles).toContain("--ui-font-size-action: 12px;");
  expect(themeStyles).toContain("--ui-font-size-control: 12.5px;");
  expect(themeStyles).toContain("--ui-font-size-heading: 13px;");

  const duplicatedTypeLiteral =
    /font-size:\s*(?:9\.5px|10\.5px|11px|11\.5px|12px|12\.5px|13px|0\.6rem|0\.62rem|0\.64rem|0\.65rem|0\.66rem|0\.68rem|0\.7rem|0\.72rem|0\.75rem|0\.76rem|0\.78rem)/;

  for (const styles of [
    sharedStyles,
    welcomeStyles,
    newProjectStyles,
    previewStyles,
  ]) {
    expect(styles).not.toMatch(duplicatedTypeLiteral);
  }
});

test("shares regular action and footer metrics", () => {
  expect(themeStyles).toContain("--ui-control-height: 31px;");
  expect(themeStyles).toContain("--ui-control-padding-inline: 15px;");
  expect(themeStyles).toContain("--ui-footer-padding: 11px 14px;");
  expect(sharedStyles).not.toMatch(
    /\.ui-dialog-window__footer\s+\.ui-action-button\s*\{/,
  );
  expect(newProjectStyles).not.toMatch(
    /\.new-project-footer\s+\.ui-action-button\s*\{/,
  );
});

test("provides one shared lined section heading", () => {
  expect(sharedStyles).toMatch(/\.ui-section-eyebrow\s*\{/);
  expect(welcomeStyles).not.toContain(".global-section-heading");
  expect(newProjectStyles).not.toMatch(
    /\.new-project-scope-label,\s*\n\.new-project-group-eyebrow/g,
  );
});
