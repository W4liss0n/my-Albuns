// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readdirSync, readFileSync } from "node:fs";

import { expect, test } from "vitest";

const readStyles = (path: string) => readFileSync(path, "utf8") as string;

function discoverStylePaths(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(
    (entry: { isDirectory(): boolean; name: string }) => {
      const path = `${directory}/${entry.name}`;
      if (entry.isDirectory()) return discoverStylePaths(path);
      return entry.name.endsWith(".css") ? [path] : [];
    },
  );
}

const sharedVisualPreviewSources = [
  "src/ui/visualPreview/PersonalizationPreview.tsx",
  "src/project/inspector/VisualScopePreview.tsx",
  "src/ui/visualPreview/ProportionalPreviewViewport.tsx",
  "src/project/inspector/VisualScopePreview.css",
  "src/ui/visualPreview/VisualPreviewSheet.css",
  "src/ui/visualPreview/ProportionalPreviewViewport.css",
].map((path) => ({ path, source: readStyles(path) }));
const applicationStyles = discoverStylePaths("src")
  .filter((path) => path !== "src/ui/theme.css")
  .map((path) => ({ path, styles: readStyles(path) }));

test("centralizes the shared type scale used by every application surface", () => {
  const duplicatedTypeLiteral =
    /font-size:\s*(?:9\.5px|10\.5px|11px|11\.5px|12px|12\.5px|13px|0\.6rem|0\.62rem|0\.64rem|0\.65rem|0\.66rem|0\.68rem|0\.7rem|0\.72rem|0\.75rem|0\.76rem|0\.78rem)/;

  expect(applicationStyles.length).toBeGreaterThan(0);
  for (const { path, styles } of applicationStyles) {
    expect(styles, path).not.toMatch(duplicatedTypeLiteral);
  }
});

test("shares only the equivalent chrome of floating surfaces", () => {
  const popups = [
    {
      source: "src/project/inspector/DecorativeMediaPicker.tsx",
      styles: "src/project/inspector/DecorativeMediaPicker.css",
      popup: "visual-design-popup",
    },
    {
      source: "src/project/media-panel/MediaPanelToolbar.tsx",
      styles: "src/project/media-panel/MediaPanel.css",
      popup: "media-popup",
    },
    {
      source: "src/global/NewProjectFlow.tsx",
      styles: "src/global/NewProjectFlow.css",
      popup: "new-project-save-preset",
    },
    {
      source: "src/project/workspace/ApplicationMenuBar.tsx",
      styles: "src/project/workspace/ApplicationMenuBar.css",
      popup: "app-menu-popup",
    },
  ];

  for (const { source, styles, popup } of popups) {
    expect(readStyles(source), source).toMatch(
      new RegExp(
        `className="(?=[^"]*\\bui-floating-surface\\b)[^"]*\\b${popup}\\b`,
      ),
    );
    expect(readStyles(styles), styles).not.toMatch(
      new RegExp(
        `\\.${popup}\\s*\\{[^}]*(?:background|box-shadow|border:|border-radius:)`,
        "s",
      ),
    );
  }
});

test("keeps the shared visual preview neutral from New Project chrome", () => {
  for (const { path, source } of sharedVisualPreviewSources) {
    expect(source, path).not.toContain("new-project-");
    expect(source, path).not.toMatch(/from\s+["'][^"']*global\//);
  }
});
