// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { existsSync, readFileSync } from "node:fs";

import { expect, test } from "vitest";

const source = (path: string) => readFileSync(path, "utf8") as string;

test("keeps feature CSS with its rendering owner", () => {
  const owners = [
    ["src/project/workspace/ApplicationMenuBar.tsx", "./ApplicationMenuBar.css"],
    ["src/project/canvas/AlbumCanvas.tsx", "./AlbumCanvas.css"],
    [
      "src/project/canvas/CanvasHorizontalScrollbar.tsx",
      "./CanvasHorizontalScrollbar.css",
    ],
    ["src/project/sheets/SheetPreview.tsx", "./SheetPreview.css"],
    ["src/global/EditorUnavailableNotice.tsx", "./EditorUnavailableNotice.css"],
    ["src/project/workspace/workspacePanelLayout.tsx", "./WorkspacePanelLayout.css"],
  ] as const;

  for (const [owner, stylesheet] of owners) {
    expect(source(owner), owner).toContain(`import "${stylesheet}";`);
  }
});

test("keeps visual preview structure with its direct owner", () => {
  expect(source("src/project/inspector/VisualScopePreview.tsx"))
    .toContain('import "./VisualScopePreview.css";');
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
    "src/project/inspector/VisualDesignControl.tsx",
    "src/project/inspector/DecorativeMediaPicker.tsx",
  ]) {
    expect(source(owner), owner).toContain('import "./VisualDesignControl.css";');
  }
  expect(source("src/project/inspector/AlbumDesignForm.css")).not.toMatch(
    /^\.visual-design-picker__(?:option|tile)\s*\{/m,
  );
});

test("makes the shared media card own its wrapper protocol", () => {
  expect(source("src/project/media-panel/MediaPreviewCard.tsx")).toContain(
    'import "./MediaPreviewCard.css";',
  );
  expect(source("src/project/media-panel/MediaThumbnail.css")).not.toMatch(
    /\.media-preview-card\b/,
  );
  expect(source("src/project/media-panel/MediaPreviewCard.tsx")).not.toContain(
    "thumbnailClassName",
  );
  for (const caller of [
    "src/project/media-panel/MediaPanel.tsx",
    "src/project/inspector/DecorativeMediaPicker.tsx",
  ]) {
    expect(source(caller), caller).toContain("<MediaPreviewCard");
    expect(source(caller), caller).not.toContain('className="media-preview-card');
  }
  expect(source("src/project/inspector/DecorativeMediaPicker.css")).not.toContain(
    "visual-design-card",
  );
});

test("keeps destructive button styling private to confirmation dialogs", () => {
  expect(source("src/ui/ActionButton.tsx")).not.toContain('"danger"');
  expect(source("src/ui/ConfirmationDialog.tsx")).toContain(
    'import "./ConfirmationDialog.css";',
  );
  expect(source("src/ui/ui.css")).not.toContain("ui-action-button--danger");
});

test("keeps App.css restricted to application-level composition", () => {
  const styles = source("src/project/App.css");

  expect(styles).not.toMatch(
    /\.(?:app-menu|canvas-shell|canvas-host|canvas-horizontal-scrollbar|sheet-preview|safe-application-shell|workspace-splitter)\b/,
  );
});

test("entrypoints import only their global foundation and owned composition", () => {
  const entrypoint = source("src/global/main.tsx");
  expect(entrypoint).not.toContain("App.css");
  expect(entrypoint).toMatch(/ui\/theme\.css/);
  expect(entrypoint).toMatch(/ui\/ui\.css/);
  expect(source("src/project/App.tsx")).toContain('import "./App.css";');
});

test("keeps form-specific inspector CSS out of the panel owner", () => {
  const styles = source("src/project/inspector/InspectorPanel.css");

  expect(styles).not.toMatch(
    /\.(?:album-information|album-entry|album-measurement|album-design|visual-default|album-frame-border)\b/,
  );
});
