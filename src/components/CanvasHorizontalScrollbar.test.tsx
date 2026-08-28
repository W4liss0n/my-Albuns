import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import type { ContinuousCanvasLayout } from "./canvasGeometry";
import { CanvasHorizontalScrollbar } from "./CanvasHorizontalScrollbar";

const layout: ContinuousCanvasLayout = {
  centeredOffset: (sheetId) =>
    ({ "sheet-1": 600, "sheet-2": 0, "sheet-3": -600 })[sheetId] ?? null,
  centeredSheetId: () => "sheet-2",
  clampOffset: (offsetX) => offsetX,
  entriesAtScale: () => [
    { sheetId: "sheet-1", index: 0, left: 0, width: 600, center: 300, right: 600 },
    { sheetId: "sheet-2", index: 1, left: 646, width: 600, center: 946, right: 1246 },
    { sheetId: "sheet-3", index: 2, left: 1292, width: 600, center: 1592, right: 1892 },
  ],
  offsetBounds: () => ({ minimum: -600, maximum: 600 }),
};

describe("CanvasHorizontalScrollbar", () => {
  test("renders a persistent explicit indicator over the latest logical position", () => {
    const { container } = render(
      <CanvasHorizontalScrollbar
        centeredSheetId="sheet-2"
        layout={layout}
        metrics={{ scale: 1, width: 600 }}
        mode={{ kind: "normal" }}
        viewport={{ offsetX: 0 }}
        onCenteredSheetChange={vi.fn()}
        onViewportChange={vi.fn()}
      />,
    );

    const thumb = container.querySelector<HTMLElement>(
      ".canvas-horizontal-scrollbar__thumb",
    );
    expect(thumb).toBeInTheDocument();
    expect(thumb).toHaveStyle({
      left: "200px",
      width: "200px",
    });
  });

  test("centers the previous and next physical Sheet through explicit controls", () => {
    const onCenteredSheetChange = vi.fn();
    const onViewportChange = vi.fn();
    render(
      <CanvasHorizontalScrollbar
        centeredSheetId="sheet-2"
        layout={layout}
        metrics={{ scale: 1, width: 600 }}
        mode={{ kind: "normal" }}
        viewport={{ offsetX: 0 }}
        onCenteredSheetChange={onCenteredSheetChange}
        onViewportChange={onViewportChange}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Lâmina anterior" }));
    expect(onViewportChange).toHaveBeenLastCalledWith({ offsetX: 600 });
    expect(onCenteredSheetChange).toHaveBeenLastCalledWith("sheet-1");

    fireEvent.click(screen.getByRole("button", { name: "Próxima Lâmina" }));
    expect(onViewportChange).toHaveBeenLastCalledWith({ offsetX: -600 });
    expect(onCenteredSheetChange).toHaveBeenLastCalledWith("sheet-3");
  });

  test("blocks structural navigation controls in Edit Mode", () => {
    render(
      <CanvasHorizontalScrollbar
        centeredSheetId="sheet-2"
        layout={layout}
        metrics={{ scale: 1, width: 600 }}
        mode={{ kind: "sheet-editing", sheetId: "sheet-2" }}
        viewport={{ offsetX: 0 }}
        onCenteredSheetChange={vi.fn()}
        onViewportChange={vi.fn()}
      />,
    );

    expect(screen.getByRole("button", { name: "Lâmina anterior" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Próxima Lâmina" })).toBeDisabled();
  });
});
