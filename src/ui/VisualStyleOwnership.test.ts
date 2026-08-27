// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { existsSync, readFileSync } from "node:fs";

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

test("keeps visual preview structure with its direct owner", () => {
  expect(source("src/ui/visualPreview/PersonalizationScopeSurface.tsx"))
    .toContain('import "./PersonalizationScopeSurface.css";');
  for (const owner of [
    "src/ui/visualPreview/PersonalizationPreview.tsx",
    "src/global/DimensionsPreview.tsx",
  ]) {
    expect(source(owner), owner).toContain("VisualPreviewSheet.css");
  }
  expect(existsSync("src/ui/visualPreview/PersonalizationPreview.css")).toBe(
    false,
  );
});

test("keeps the outside-surface interaction with its sole New Project owner", () => {
  const sharedViewportSources = [
    "src/ui/visualPreview/ProportionalPreviewViewport.tsx",
    "src/ui/visualPreview/ProportionalPreviewViewport.css",
    "src/ui/visualPreview/index.ts",
  ] as const;

  for (const path of sharedViewportSources) {
    expect(source(path), path).not.toMatch(
      /PreviewOutsideSurfaceAction|outsideSurfaceAction|visual-preview-outside-action/,
    );
  }

  expect(source("src/global/NewProjectPreviewPanel.tsx")).toContain(
    "interface PreviewOutsideSurfaceAction",
  );
  expect(source("src/global/NewProjectPreviewPanel.tsx")).toContain(
    'className="new-project-preview-outside-action"',
  );
  expect(source("src/global/NewProjectPreviewPanel.css")).toContain(
    ".new-project-preview-outside-action",
  );
});

test("keeps shared visual-default option policy in a neutral module", () => {
  for (const owner of [
    "src/components/AlbumDesignForm.tsx",
    "src/components/DecorativeMediaPicker.tsx",
  ]) {
    expect(source(owner), owner).toContain('import "./VisualDefaultPicker.css";');
  }
  expect(source("src/components/AlbumDesignForm.css")).not.toMatch(
    /^\.visual-default-picker__(?:option|tile)\s*\{/m,
  );
});

test("makes the shared media card own its wrapper protocol", () => {
  expect(source("src/components/MediaPreviewCard.tsx")).toContain(
    'import "./MediaPreviewCard.css";',
  );
  expect(source("src/components/MediaThumbnail.css")).not.toMatch(
    /\.media-preview-card\b/,
  );
  expect(source("src/components/MediaPreviewCard.tsx")).not.toContain(
    "thumbnailClassName",
  );
  for (const caller of [
    "src/components/MediaPanel.tsx",
    "src/components/DecorativeMediaPicker.tsx",
  ]) {
    expect(source(caller), caller).toContain("<MediaPreviewCard");
    expect(source(caller), caller).not.toContain('className="media-preview-card');
  }
  expect(source("src/components/DecorativeMediaPicker.css")).not.toContain(
    "visual-default-card",
  );
});

test("keeps destructive button styling private to confirmation dialogs", () => {
  expect(source("src/ui/ActionButton.tsx")).not.toContain('"danger"');
  expect(source("src/ui/ConfirmationDialog.tsx")).toContain(
    'import "./ConfirmationDialog.css";',
  );
  expect(source("src/ui/ui.css")).not.toContain("ui-action-button--danger");
});

test("requires productive preview and diagnostic dependencies at editor seams", () => {
  const mediaPanel = source("src/components/MediaPanel.tsx");
  expect(mediaPanel).toContain('kind: "connected";');
  expect(mediaPanel).toContain('kind: "static";');
  expect(mediaPanel).toContain("previewSource: MediaPanelPreviewSource;");
  expect(mediaPanel).not.toContain("mediaPreviews = {}");

  const workspace = source("src/components/ProjectWorkspace.tsx");
  expect(workspace).toContain(
    "mediaPreviews: Readonly<Record<string, MediaPreview>>;",
  );
  expect(workspace).toContain(
    "onMediaDemandChange(demand: MediaPreviewDemand): void;",
  );
  expect(workspace).toContain(
    "onGraphicsUnavailable(diagnostic: GraphicsDiagnostic): void;",
  );
  expect(workspace).toContain("onPreferencesReady(projectId: string): void;");
});

test("keeps shared contracts canonical and removes dead visual protocols", () => {
  const globalProjectPort = source(
    "src/global/application/globalProjectPort.ts",
  );
  expect(globalProjectPort).toContain(
    'import type { ScopedValue } from "../../application/scopedValues";',
  );
  expect(globalProjectPort).not.toContain("InitialScopedContent");

  for (const [path, protocol] of [
    [
      "src/ui/visualPreview/PersonalizationPreview.tsx",
      "visual-personalization-preview",
    ],
    ["src/global/DimensionsPreview.tsx", "new-project-dimensions-sheet"],
    ["src/components/DecorativeMediaPicker.tsx", "visual-default-card"],
  ] as const) {
    expect(source(path), path).not.toContain(protocol);
  }
  expect(source("src/ui/visualPreview/index.ts")).not.toContain(
    "export { PersonalizationPreview }",
  );
  expect(source("src/ui/ActionButton.tsx")).not.toContain(
    "export type ActionButtonVariant",
  );
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
