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

const themeStyles = readStyles("src/ui/theme.css");
const sharedStyles = readStyles("src/ui/ui.css");
const welcomeStyles = readStyles("src/global/GlobalShell.css");
const newProjectStyles = readStyles("src/global/NewProjectFlow.css");
const newProjectPreviewStyles = readStyles(
  "src/global/NewProjectPreviewPanel.css",
);
const editorStyles = readStyles("src/App.css");
const canvasPreviewStyles = readStyles("src/canvas-preview.css");
const mediaPanelStyles = readStyles("src/components/MediaPanel.css");
const mediaThumbnailStyles = readStyles("src/components/MediaThumbnail.css");
const inspectorPanelStyles = readStyles("src/components/InspectorPanel.css");
const decorativePickerStyles = readStyles(
  "src/components/DecorativeMediaPicker.css",
);
const albumInformationSource = readStyles("src/components/AlbumInformationForm.tsx");
const decorativePickerSource = readStyles(
  "src/components/DecorativeMediaPicker.tsx",
);
const mediaToolbarSource = readStyles("src/components/MediaPanelToolbar.tsx");
const newProjectSource = readStyles("src/global/NewProjectFlow.tsx");
const newProjectPreviewSource = readStyles(
  "src/global/NewProjectPreviewPanel.tsx",
);
const newProjectPersonalizationSource = readStyles(
  "src/global/PersonalizationStep.tsx",
);
const albumDesignSource = readStyles("src/components/AlbumDesignForm.tsx");
const sharedVisualPreviewSources = [
  "src/ui/visualPreview/PersonalizationPreview.tsx",
  "src/ui/visualPreview/PersonalizationScopeSurface.tsx",
  "src/ui/visualPreview/ProportionalPreviewViewport.tsx",
  "src/ui/visualPreview/PersonalizationPreview.css",
  "src/ui/visualPreview/ProportionalPreviewViewport.css",
].map((path) => ({ path, source: readStyles(path) }));
const exportStyles = readStyles("src/components/ExportPreviewControl.css");
const projectWorkspaceSource = readStyles("src/components/ProjectWorkspace.tsx");
const sheetPreviewSource = readStyles("src/components/SheetPreview.tsx");
const canvasRenderNodesSource = readStyles(
  "src/components/albumCanvasRenderNodes.ts",
);
const exportSource = readStyles("src/components/ExportPreviewControl.tsx");
const applicationStyles = discoverStylePaths("src")
  .filter((path) => path !== "src/ui/theme.css")
  .map((path) => ({ path, styles: readStyles(path) }));

test("centralizes the shared type scale used by every application surface", () => {
  expect(themeStyles).toContain("--ui-font-size-micro: 9.5px;");
  expect(themeStyles).toContain("--ui-font-size-caption: 10.5px;");
  expect(themeStyles).toContain("--ui-font-size-support: 11px;");
  expect(themeStyles).toContain("--ui-font-size-label: 11.5px;");
  expect(themeStyles).toContain("--ui-font-size-action: 12px;");
  expect(themeStyles).toContain("--ui-font-size-control: 12.5px;");
  expect(themeStyles).toContain("--ui-font-size-heading: 13px;");

  const duplicatedTypeLiteral =
    /font-size:\s*(?:9\.5px|10\.5px|11px|11\.5px|12px|12\.5px|13px|0\.6rem|0\.62rem|0\.64rem|0\.65rem|0\.66rem|0\.68rem|0\.7rem|0\.72rem|0\.75rem|0\.76rem|0\.78rem)/;

  expect(applicationStyles.map(({ path }) => path)).toEqual(
    expect.arrayContaining([
      "src/components/DecorativeMediaPicker.css",
      "src/ui/visualPreview/PersonalizationPreview.css",
      "src/ui/visualPreview/ProportionalPreviewViewport.css",
    ]),
  );
  for (const { path, styles } of applicationStyles) {
    expect(styles, path).not.toMatch(duplicatedTypeLiteral);
  }
});

test("shares dense application chrome metrics", () => {
  expect(themeStyles).toContain(
    "--ui-commandbar-height: var(--ui-control-height);",
  );
  expect(themeStyles).toContain("--ui-toolbar-height: 34px;");
  expect(themeStyles).toContain("--ui-compact-control-height: 28px;");
  expect(editorStyles).toMatch(
    /grid-template-rows:\s*var\(--ui-titlebar-height\) var\(--ui-commandbar-height\)\s*minmax\(0, 1fr\);/,
  );
  expect(mediaPanelStyles).toContain(
    "grid-template-rows: var(--ui-toolbar-height) minmax(0, 1fr);",
  );
  expect(sharedStyles).toContain(
    "height: var(--ui-compact-control-height);",
  );
  expect(albumInformationSource).toContain('className="ui-field-control"');
  expect(albumInformationSource).not.toContain("<ActionButton");
});

test("uses the shared subtle shadow below the media toolbar", () => {
  expect(themeStyles).toContain(
    "--ui-shadow-toolbar: 0 2px 6px rgb(60 54 44 / 8%);",
  );
  expect(mediaPanelStyles).toMatch(
    /\.media-toolbar\s*\{[^}]*box-shadow:\s*var\(--ui-shadow-toolbar\);/s,
  );
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

test("shares application menus and empty states across surfaces", () => {
  expect(sharedStyles).toMatch(/\.ui-empty-state\s*\{/);
  expect(welcomeStyles).not.toMatch(/\.global-empty-state\s+(?:strong|p)\s*\{/);
  expect(projectWorkspaceSource).toContain("<ApplicationMenuBar");
});

test("shares floating notification chrome", () => {
  expect(sharedStyles).toMatch(/\.ui-inline-notice--floating\s*\{/);
  expect(projectWorkspaceSource).toContain("<InlineNotice");
  expect(projectWorkspaceSource).toContain("floating");
  expect(exportSource).toContain("floating");
  expect(exportStyles).not.toContain("position: fixed");
});

test("keeps media hover and selection on the straight image border", () => {
  expect(mediaThumbnailStyles).toMatch(
    /\.media-preview-thumbnail\s*\{[^}]*border:\s*3px solid #fff;[^}]*border-radius:\s*0;/s,
  );
  expect(mediaThumbnailStyles).toMatch(
    /\.media-preview-card:hover \.media-preview-thumbnail\s*\{\s*border-color:\s*var\(--ui-border-strong\);\s*\}/,
  );
  expect(mediaThumbnailStyles).toMatch(
    /\.media-preview-card\[data-selected="true"\] \.media-preview-thumbnail\s*\{\s*border-color:\s*var\(--ui-accent\);\s*\}/,
  );
  expect(mediaThumbnailStyles).toMatch(
    /\.media-preview-thumbnail img\s*\{[^}]*object-fit:\s*contain;/s,
  );
  expect(mediaPanelStyles).toContain("user-select: none;");
  expect(mediaThumbnailStyles).not.toMatch(
    /\.media-preview-card:hover \.media-preview-thumbnail\s*\{[^}]*box-shadow/s,
  );
  expect(mediaThumbnailStyles).toMatch(
    /\.media-preview-card:focus-visible \.media-preview-thumbnail\s*\{[^}]*outline:\s*1px solid var\(--ui-text-muted\);/s,
  );
  expect(mediaThumbnailStyles).not.toMatch(
    /\.media-preview-card:focus(?:-visible)? \.media-preview-thumbnail\s*\{[^}]*border-color:/s,
  );
});

test("shares only the equivalent chrome of floating surfaces", () => {
  expect(sharedStyles).toMatch(/\.ui-floating-surface\s*\{/);
  expect(sharedStyles).toMatch(
    /\.ui-floating-surface\s*\{[^}]*border:\s*1px solid var\(--ui-border-strong\);[^}]*border-radius:\s*var\(--ui-radius\);[^}]*background:\s*var\(--ui-surface-raised\);[^}]*box-shadow:\s*var\(--ui-shadow-menu\);/s,
  );
  expect(decorativePickerSource).toContain(
    'className="ui-floating-surface visual-default-popup"',
  );
  expect(mediaToolbarSource).toContain(
    'className="ui-floating-surface media-popup media-import-popup"',
  );
  expect(mediaToolbarSource).toContain(
    'className="ui-floating-surface media-popup media-options-popup"',
  );
  expect(newProjectSource).toContain(
    'className="ui-floating-surface new-project-save-preset"',
  );
  expect(decorativePickerStyles).not.toMatch(
    /\.visual-default-popup\s*\{[^}]*(?:background|box-shadow|border:)/s,
  );
  expect(mediaPanelStyles).not.toMatch(
    /\.media-popup\s*\{[^}]*(?:background|box-shadow|border:)/s,
  );
  expect(newProjectStyles).not.toMatch(
    /\.new-project-save-preset\s*\{[^}]*(?:background|box-shadow|border:)/s,
  );
});

test("uses the canonical media fallback in SVG and Pixi Sheet renderers", () => {
  expect(sheetPreviewSource).toContain(
    "SHEET_VISUAL_STYLE.mediaFallback.fill",
  );
  expect(canvasRenderNodesSource).toContain(
    "pixiColor(SHEET_VISUAL_STYLE.mediaFallback.fill)",
  );
  expect(sheetPreviewSource).not.toMatch(/#D8DEE2/i);
  expect(canvasRenderNodesSource).not.toMatch(/0xd8dee2/i);
});

test("uses the canonical Sheet guide colors in the New Project legend", () => {
  expect(newProjectPreviewSource).toContain("SHEET_GUIDE_STYLE.bleed");
  expect(newProjectPreviewSource).toContain("SHEET_GUIDE_STYLE.safety");
  expect(newProjectPreviewStyles).toContain(
    "var(--new-project-guide-bleed)",
  );
  expect(newProjectPreviewStyles).toContain(
    "var(--new-project-guide-safety)",
  );
  expect(newProjectPreviewStyles).not.toMatch(/#c57c70|#6f9fbe/i);
});

test("keeps the shared visual preview neutral from New Project chrome", () => {
  for (const { path, source } of sharedVisualPreviewSources) {
    expect(source, path).not.toContain("new-project-");
    expect(source, path).not.toMatch(/from\s+["'][^"']*global\//);
  }
  expect(newProjectPersonalizationSource).toContain(
    'from "../ui/visualPreview"',
  );
  expect(albumDesignSource).toContain('from "../ui/visualPreview"');
  expect(albumDesignSource).not.toMatch(/from\s+["'][^"']*global\//);
});

test("keeps compact visual-default focus independent from selection", () => {
  expect(inspectorPanelStyles).toMatch(
    /\.visual-default-picker__option:focus-visible \.visual-default-picker__tile,[\s\S]*outline:\s*1px solid var\(--ui-text-muted\);[\s\S]*outline-offset:\s*2px;/,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.visual-default-picker__option\[data-selected="true"\][\s\S]*border-color:\s*var\(--ui-accent\);/,
  );
});

test("matches the compact sheet grid instead of using generic cards", () => {
  expect(themeStyles).toContain(
    "--ui-shadow-thumbnail: 0 1px 2px rgb(60 54 44 / 10%);",
  );
  expect(inspectorPanelStyles).toMatch(
    /\.sheet-grid\s*\{[^}]*grid-template-columns:\s*repeat\(2, minmax\(0, 1fr\)\);/s,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.sheet-tile\s*\{[^}]*position:\s*relative;[^}]*padding:\s*0;[^}]*overflow:\s*hidden;[^}]*border:\s*0;[^}]*border-radius:\s*0;[^}]*aspect-ratio:\s*2;/s,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.sheet-tile\.active\s*\{[^}]*box-shadow:\s*0 0 0 2px var\(--ui-accent\),\s*var\(--ui-shadow-thumbnail\);/s,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.sheet-tile:hover:not\(\.active\)\s*\{[^}]*box-shadow:\s*0 0 0 2px var\(--ui-border-strong\),\s*var\(--ui-shadow-thumbnail\);/s,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.sheet-tile:hover:not\(\.active\) \.sheet-tile__number,\s*\n\.sheet-tile:hover:not\(\.active\) \.sheet-tile__pages\s*\{[^}]*color:\s*var\(--ui-text\);[^}]*background:\s*var\(--ui-border-strong\);/s,
  );
  expect(inspectorPanelStyles).not.toMatch(
    /\.sheet-tile\.active\s*\{[^}]*background:\s*var\(--ui-accent-soft\);/s,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.sheet-tile__number,\s*\n\.sheet-tile__pages\s*\{[^}]*position:\s*absolute;/s,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.sheet-tile\.active \.sheet-tile__number,\s*\n\.sheet-tile\.active \.sheet-tile__pages\s*\{[^}]*color:\s*#fff;[^}]*background:\s*var\(--ui-accent\);/s,
  );
});

test("integrates read-only Album information without making editable controls look passive", () => {
  expect(inspectorPanelStyles).toMatch(
    /\.inspector-readout--integrated\s*\{[^}]*border:\s*0;[^}]*background:\s*transparent;/s,
  );
  expect(inspectorPanelStyles).not.toMatch(
    /\.inspector-readout--field-placeholder\s*\{[^}]*border:\s*0;/s,
  );
  expect(inspectorPanelStyles).toMatch(
    /\.document-compact-controls\s*\{[^}]*grid-template-columns:\s*72px minmax\(0, 1fr\);/s,
  );
});

test("lets the continuous Canvas preview use the complete available height", () => {
  expect(canvasPreviewStyles).toMatch(
    /\.canvas-preview\s*\{[^}]*height:\s*100%;/s,
  );
  expect(canvasPreviewStyles).not.toContain("420px");
});

test("uses the focused Sheet instead of the Canvas perimeter as the keyboard focus indicator", () => {
  expect(editorStyles).toMatch(
    /\.pixi-canvas:focus-visible\s*\{[^}]*outline:\s*none;/s,
  );
  expect(editorStyles).toMatch(
    /\.canvas-horizontal-scrollbar:focus-visible\s*\{[^}]*background:\s*var\(--ui-surface-muted\);/s,
  );
  expect(editorStyles).not.toMatch(
    /\.pixi-canvas:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--ui-accent\);/s,
  );
});
