// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { expect, test } from "vitest";

const source = (path: string) => readFileSync(path, "utf8") as string;

test("keeps feature CSS with its rendering owner", () => {
  const owners = [
    ["src/components/ApplicationMenuBar.tsx", "./ApplicationMenuBar.css"],
    ["src/components/AlbumCanvas.tsx", "./AlbumCanvas.css"],
    [
      "src/components/CanvasHorizontalScrollbar.tsx",
      "./CanvasHorizontalScrollbar.css",
    ],
    ["src/components/SheetPreview.tsx", "./SheetPreview.css"],
    ["src/components/SafeApplicationShell.tsx", "./SafeApplicationShell.css"],
    ["src/components/workspacePanelLayout.tsx", "./WorkspacePanelLayout.css"],
  ] as const;

  for (const [owner, stylesheet] of owners) {
    expect(source(owner), owner).toContain(`import "${stylesheet}";`);
  }
});

test("keeps App.css restricted to application-level composition", () => {
  const styles = source("src/App.css");

  expect(styles).not.toMatch(
    /\.(?:app-menu|canvas-shell|canvas-host|canvas-horizontal-scrollbar|sheet-preview|safe-application-shell|workspace-splitter)\b/,
  );
});

test("entrypoints import only their global foundation and owned composition", () => {
  for (const entrypoint of [
    "src/canvas-preview.tsx",
    "src/media-panel-preview.tsx",
    "src/sheet-grid-preview.tsx",
    "src/ui-acceptance-preview.tsx",
    "src/welcome-preview.tsx",
    "src/global/main.tsx",
  ]) {
    const contents = source(entrypoint);
    expect(contents, entrypoint).not.toContain("App.css");
    expect(contents, entrypoint).toMatch(/ui\/theme\.css/);
    expect(contents, entrypoint).toMatch(/ui\/ui\.css/);
  }
  expect(source("src/App.tsx")).toContain('import "./App.css";');
});

test("keeps form-specific inspector CSS out of the panel owner", () => {
  const styles = source("src/components/InspectorPanel.css");

  expect(styles).not.toMatch(
    /\.(?:album-information|album-entry|album-measurement|album-design|visual-default|album-frame-border)\b/,
  );
});

test("composes the shared floating chrome into application menus", () => {
  const menuSource = source("src/components/ApplicationMenuBar.tsx");

  expect(menuSource).toContain(
    'className="ui-floating-surface app-menu-popup"',
  );
  expect(menuSource).toContain(
    'className="ui-floating-surface app-menu-popup app-menu-submenu-popup"',
  );
  expect(source("src/components/ApplicationMenuBar.css")).not.toMatch(
    /\.app-menu-popup\s*\{[^}]*(?:border:|border-radius:|background:|box-shadow:)/s,
  );
});
