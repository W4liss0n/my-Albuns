import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { AlbumCanvasProps } from "./components/albumCanvasContract";
import { CanvasPreview } from "./canvas-preview";

vi.mock("./components/AlbumCanvas", () => ({
  AlbumCanvas: ({ mode, onEditSheet }: AlbumCanvasProps) => (
    <div className="canvas-host">
      <canvas
        data-testid="canvas-preview-surface"
        data-mode={mode.kind}
        data-sheet={mode.kind === "sheet-editing" ? mode.sheetId : undefined}
        onDoubleClick={() => onEditSheet("sheet-002")}
        tabIndex={0}
      />
    </div>
  ),
}));

afterEach(() => {
  window.history.replaceState({}, "", "/");
});

test("enters Sheet Edit Mode with Enter while the preview Canvas has focus", () => {
  window.history.replaceState({}, "", "/canvas-preview.html");
  render(<CanvasPreview />);
  const canvas = screen.getByTestId("canvas-preview-surface");

  canvas.focus();
  fireEvent.keyDown(canvas, { key: "Enter" });

  expect(canvas).toHaveAttribute("data-mode", "sheet-editing");
  expect(canvas).toHaveAttribute("data-sheet", "sheet-002");
});

test("returns to normal mode with Escape from the preview Sheet Edit Mode", () => {
  window.history.replaceState(
    {},
    "",
    "/canvas-preview.html?mode=sheet-editing&sheet=sheet-002",
  );
  render(<CanvasPreview />);
  const canvas = screen.getByTestId("canvas-preview-surface");
  expect(canvas).toHaveAttribute("data-mode", "sheet-editing");

  fireEvent.keyDown(window, { key: "Escape" });

  expect(canvas).toHaveAttribute("data-mode", "normal");
});
