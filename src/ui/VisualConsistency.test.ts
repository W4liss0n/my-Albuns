// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { expect, test } from "vitest";

const readStyles = (path: string) => readFileSync(path, "utf8") as string;

const themeStyles = readStyles("src/ui/theme.css");
const sharedStyles = readStyles("src/ui/ui.css");
const welcomeStyles = readStyles("src/global/GlobalShell.css");
const newProjectStyles = readStyles("src/global/NewProjectFlow.css");
const previewStyles = readStyles("src/global/NewProjectPreviewPanel.css");
const editorStyles = readStyles("src/App.css");
const canvasPreviewStyles = readStyles("src/canvas-preview.css");
const mediaPanelStyles = readStyles("src/components/MediaPanel.css");
const inspectorPanelStyles = readStyles("src/components/InspectorPanel.css");
const documentDpiSource = readStyles("src/components/DocumentDpiControl.tsx");
const exportStyles = readStyles("src/components/ExportPreviewControl.css");
const projectWorkspaceSource = readStyles("src/components/ProjectWorkspace.tsx");
const exportSource = readStyles("src/components/ExportPreviewControl.tsx");

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

  for (const styles of [
    sharedStyles,
    welcomeStyles,
    newProjectStyles,
    previewStyles,
    editorStyles,
    mediaPanelStyles,
    inspectorPanelStyles,
    exportStyles,
  ]) {
    expect(styles).not.toMatch(duplicatedTypeLiteral);
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
  expect(documentDpiSource).toContain('className="ui-field-control"');
  expect(documentDpiSource).not.toContain("<ActionButton");
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
  expect(mediaPanelStyles).toMatch(
    /\.media-thumb\s*\{[^}]*border:\s*3px solid #fff;[^}]*border-radius:\s*0;/s,
  );
  expect(mediaPanelStyles).toMatch(
    /\.media-card:hover \.media-thumb\s*\{\s*border-color:\s*var\(--ui-border-strong\);\s*\}/,
  );
  expect(mediaPanelStyles).toMatch(
    /\.media-card\[data-selected="true"\] \.media-thumb\s*\{\s*border-color:\s*var\(--ui-accent\);\s*\}/,
  );
  expect(mediaPanelStyles).toMatch(
    /\.media-thumb img\s*\{[^}]*object-fit:\s*contain;/s,
  );
  expect(mediaPanelStyles).toContain("user-select: none;");
  expect(mediaPanelStyles).not.toMatch(
    /\.media-card:hover \.media-thumb\s*\{[^}]*box-shadow/s,
  );
  expect(mediaPanelStyles).toMatch(
    /\.media-card:focus-visible \.media-thumb\s*\{[^}]*outline:\s*1px solid var\(--ui-text-muted\);/s,
  );
  expect(mediaPanelStyles).not.toMatch(
    /\.media-card:focus(?:-visible)? \.media-thumb\s*\{[^}]*border-color:/s,
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
  expect(editorStyles).not.toMatch(
    /\.pixi-canvas:focus-visible\s*\{[^}]*outline:\s*2px solid var\(--ui-accent\);/s,
  );
});
