// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import {
  useWorkspacePanelLayout,
  WorkspacePanelSplitter,
} from "./workspacePanelLayout";

function SplitterGeometryHarness() {
  const layout = useWorkspacePanelLayout({
    preferences: { inspector: null, media: null },
    onSizeChange: vi.fn(),
    onVisibilityChange: vi.fn(),
  });

  return (
    <div data-testid="workspace" style={layout.style}>
      <WorkspacePanelSplitter
        onResizeBy={vi.fn()}
        onResizeStart={vi.fn()}
        panel="inspector"
        size={layout.panels.inspector.size}
      />
      <WorkspacePanelSplitter
        onResizeBy={vi.fn()}
        onResizeStart={vi.fn()}
        panel="media"
        size={layout.panels.media.size}
      />
    </div>
  );
}

test("reserves only the semantic hairline for each visible splitter", () => {
  render(<SplitterGeometryHarness />);

  const style = screen.getByTestId("workspace").style;
  expect(style.getPropertyValue("--inspector-splitter-size")).toBe(
    "var(--ui-splitter-visual-size)",
  );
  expect(style.getPropertyValue("--media-splitter-size")).toBe(
    "var(--ui-splitter-visual-size)",
  );
});

test("separates the one-pixel visual line from a broad overlay hit target", () => {
  const theme = readFileSync("src/ui/theme.css", "utf8") as string;
  const styles = readFileSync(
    "src/components/WorkspacePanelLayout.css",
    "utf8",
  ) as string;

  expect(theme).toMatch(/--ui-splitter-visual-size:\s*1px/);
  expect(styles).toContain(".workspace-splitter::before");
  expect(styles).toMatch(
    /\.inspector-splitter::before\s*\{[^}]*width:\s*var\(--ui-space-3\)/s,
  );
  expect(styles).toMatch(
    /\.media-splitter::before\s*\{[^}]*height:\s*var\(--ui-space-3\)/s,
  );
});

test("retains separator semantics and keyboard resize on both axes", () => {
  const resizeBy = vi.fn();
  render(
    <>
      <WorkspacePanelSplitter
        onResizeBy={resizeBy}
        onResizeStart={vi.fn()}
        panel="inspector"
        size={310}
      />
      <WorkspacePanelSplitter
        onResizeBy={resizeBy}
        onResizeStart={vi.fn()}
        panel="media"
        size={202}
      />
    </>,
  );

  const inspector = screen.getByRole("separator", {
    name: "Redimensionar Painel contextual",
  });
  const media = screen.getByRole("separator", {
    name: "Redimensionar Painel de imagens",
  });
  expect(inspector).toHaveAttribute("aria-orientation", "vertical");
  expect(media).toHaveAttribute("aria-orientation", "horizontal");
  fireEvent.keyDown(inspector, { key: "ArrowLeft" });
  fireEvent.keyDown(media, { key: "ArrowUp" });
  expect(resizeBy).toHaveBeenNthCalledWith(1, "inspector", 12);
  expect(resizeBy).toHaveBeenNthCalledWith(2, "media", 12);
});
