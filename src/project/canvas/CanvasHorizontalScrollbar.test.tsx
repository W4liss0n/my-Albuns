import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

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
        metrics={{ scale: 1, width: 600, height: 500 }}
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
  afterEach(() => vi.restoreAllMocks());

  test("follows wheel steps once per frame and ignores the echo of its own write", () => {
    const frames: FrameRequestCallback[] = [];
    vi.spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => frames.push(callback));
    const onViewportChange = vi.fn();
    const scrollbar = (offsetX: number) => (
      <CanvasHorizontalScrollbar
        centeredSheetId="sheet-2"
        layout={layout}
        metrics={{ scale: 1, width: 600, height: 500 }}
        mode={{ kind: "normal" }}
        viewport={{ offsetX }}
        onCenteredSheetChange={vi.fn()}
        onViewportChange={onViewportChange}
      />
    );
    const view = render(scrollbar(0));
    const scroller = screen.getByRole("scrollbar");
    let scrollLeft = 0;
    const writes: number[] = [];
    Object.defineProperty(scroller, "scrollLeft", {
      configurable: true,
      get: () => scrollLeft,
      set: (value: number) => { writes.push(value); scrollLeft = value; },
    });
    frames.splice(0).forEach((frame) => frame(0));
    expect(writes).toEqual([600]);

    view.rerender(scrollbar(-100));
    view.rerender(scrollbar(-200));
    expect(writes).toEqual([600]);
    // The scroll event of the first write lands after the newer steps.
    fireEvent.scroll(scroller);
    expect(onViewportChange).not.toHaveBeenCalled();

    expect(frames).toHaveLength(1);
    frames.splice(0).forEach((frame) => frame(0));
    expect(writes).toEqual([600, 800]);

    scrollLeft = 700;
    fireEvent.scroll(scroller);
    expect(onViewportChange).toHaveBeenCalledExactlyOnceWith({ offsetX: -100 });
  });
});
