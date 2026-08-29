import { render, screen } from "@testing-library/react";
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

  test("does not render permanent previous or next Sheet controls", () => {
    render(
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

    expect(
      screen.queryByRole("button", { name: "Lâmina anterior" }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Próxima Lâmina" }),
    ).not.toBeInTheDocument();
  });
});
