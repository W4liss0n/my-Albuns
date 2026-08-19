import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";

import type { AlbumCanvasProps } from "./components/albumCanvasContract";
import { CanvasPreview } from "./canvas-preview";

vi.mock("./components/AlbumCanvas", () => ({
  AlbumCanvas: ({
    centeredSheetId,
    mode,
    onCanvasMetricsChange,
    onEditSheet,
    onSelectFrame,
    selectedFrameId,
    viewport,
  }: AlbumCanvasProps) => (
    <div className="canvas-host">
      <canvas
        data-centered-sheet={centeredSheetId ?? undefined}
        data-testid="canvas-preview-surface"
        data-mode={mode.kind}
        data-offset-x={viewport.offsetX}
        data-selected-frame={selectedFrameId ?? undefined}
        data-sheet={mode.kind === "sheet-editing" ? mode.sheetId : undefined}
        onDoubleClick={() => onEditSheet("sheet-002")}
        tabIndex={0}
      />
      <button
        data-testid="report-canvas-metrics"
        onClick={() => onCanvasMetricsChange?.({ width: 1_000, scale: 0.5 })}
        type="button"
      />
      <button
        data-testid="select-preview-frame"
        onClick={() => onSelectFrame("sheet-002-top")}
        type="button"
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

test("centers the edited Sheet when the preview returns to normal mode", () => {
  window.history.replaceState(
    {},
    "",
    "/canvas-preview.html?mode=sheet-editing&sheet=sheet-003",
  );
  render(<CanvasPreview />);
  const canvas = screen.getByTestId("canvas-preview-surface");

  fireEvent.click(screen.getByTestId("report-canvas-metrics"));
  expect(canvas).toHaveAttribute("data-centered-sheet", "sheet-002");
  const initialOffset = canvas.getAttribute("data-offset-x");
  expect(initialOffset).not.toBeNull();

  fireEvent.keyDown(window, { key: "Escape" });

  expect(canvas).toHaveAttribute("data-mode", "normal");
  expect(canvas).toHaveAttribute("data-centered-sheet", "sheet-002");
  expect(canvas).toHaveAttribute("data-offset-x", initialOffset);

  fireEvent.click(screen.getByTestId("report-canvas-metrics"));

  expect(canvas).toHaveAttribute("data-centered-sheet", "sheet-003");
  expect(canvas.getAttribute("data-offset-x")).not.toBe(initialOffset);
});

test("keeps Frame selection state in the development preview", () => {
  window.history.replaceState(
    {},
    "",
    "/canvas-preview.html?mode=sheet-editing&sheet=sheet-002",
  );
  render(<CanvasPreview />);
  const canvas = screen.getByTestId("canvas-preview-surface");

  fireEvent.click(screen.getByTestId("select-preview-frame"));

  expect(canvas).toHaveAttribute("data-selected-frame", "sheet-002-top");
});
