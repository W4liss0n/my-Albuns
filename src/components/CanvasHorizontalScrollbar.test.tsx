import { render } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";

import type { ContinuousCanvasLayout } from "./canvasGeometry";
import { CanvasHorizontalScrollbar } from "./CanvasHorizontalScrollbar";

const layout: ContinuousCanvasLayout = {
  centeredOffset: () => 0,
  centeredSheetId: () => "sheet-2",
  clampOffset: (offsetX) => offsetX,
  entriesAtScale: () => [],
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
});
