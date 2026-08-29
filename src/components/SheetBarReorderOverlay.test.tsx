import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import type { ComposedSheet } from "../domain/project";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import { SheetBarReorderOverlay } from "./SheetBarReorderOverlay";

const sheets = [sheet("sheet-1", 1), sheet("sheet-2", 2)];
const layout = createContinuousCanvasLayout(sheets);

test("aligns enabled pointer handles with the scaled Sheet Bar slots", () => {
  const onPreview = vi.fn();
  const { rerender } = render(
    <SheetBarReorderOverlay {...props({ onPreview })} />,
  );
  const overlay = screen.getByRole("group", {
    name: "Reordenação pela Barra da Lâmina",
  });
  const first = barHandle(1);
  const second = barHandle(2);

  expect(first).toHaveStyle({ left: "25px", top: "28px", width: "100px" });
  expect(second).toHaveStyle({ left: "171px", top: "28px", width: "100px" });
  expect(first).not.toHaveAttribute("draggable");
  expect(overlay).toHaveAttribute("aria-disabled", "false");

  rerender(<SheetBarReorderOverlay {...props({ onPreview })} disabled />);
  expect(overlay).toHaveAttribute("aria-disabled", "true");
  pointerDown(first, 1, 100, 40);
  pointerMove(first, 1, 240, 40);
  expect(onPreview).not.toHaveBeenCalled();

  rerender(
    <SheetBarReorderOverlay
      {...props({ onPreview, status: "committing" })}
    />,
  );
  expect(overlay).toHaveAttribute("aria-disabled", "true");
});

test.each([
  ["left page", 50],
  ["right page", 100],
] as const)(
  "starts pointer reordering from the %s of the visible Sheet Bar",
  (_side, startX) => {
    const onCancel = vi.fn();
    const onPreview = vi.fn();
    const view = render(
      <SheetBarReorderOverlay
        {...props({ onCancel, onPreview })}
      />,
    );
    setOverlayBounds();
    const surface = barSurface();
    const handle = barHandle(1);
    pointerCapture(surface);

    expectPointerCoordinateInsideHandle(handle, startX, 40);
    pointerDown(handle, 70 + startX, startX, 40);
    pointerMove(surface, 70 + startX, 240, 40);

    expect(onPreview).toHaveBeenLastCalledWith("sheet-1", 1);
    fireEvent.pointerCancel(surface, { pointerId: 70 + startX });
    expect(onCancel).toHaveBeenCalledOnce();
    view.unmount();
  },
);

test("ignores pointer gestures outside every rendered Sheet Bar handle", () => {
  const onNavigate = vi.fn();
  const onPreview = vi.fn();
  render(
    <SheetBarReorderOverlay {...props({ onNavigate, onPreview })} />,
  );
  const reservedSurface = screen.getByTestId(
    "sheet-reorder-bar-drop-zone",
  );

  fireEvent.pointerDown(reservedSurface, { button: 0, pointerId: 1 });
  fireEvent.pointerMove(reservedSurface, {
    clientX: 240,
    pointerId: 1,
  });
  fireEvent.click(reservedSurface);
  expect(onPreview).not.toHaveBeenCalled();
  expect(onNavigate).not.toHaveBeenCalled();

  fireEvent.click(barHandle(1));
  expect(onNavigate).toHaveBeenCalledWith("sheet-1");
});

test("moves neighboring handles and renders only the declared preview markers", () => {
  const { rerender } = render(<SheetBarReorderOverlay {...props()} />);
  expect(screen.queryByTestId("reorder-placeholder")).not.toBeInTheDocument();
  expect(screen.queryByTestId("reorder-ghost")).not.toBeInTheDocument();

  rerender(
    <SheetBarReorderOverlay
      {...props({
        representation: {
          ghost: { sheetId: "sheet-2" },
          order: ["sheet-2", "sheet-1"],
          placeholderIndex: 1,
        },
        status: "preview",
      })}
    />,
  );

  expect(barHandle(1)).toHaveStyle({ left: "171px" });
  expect(barHandle(1)).toHaveAttribute("data-reorder-shift", "true");
  expect(barHandle(2)).toHaveStyle({ left: "25px" });
  expect(barHandle(2)).toHaveAttribute("data-reorder-ghost", "true");
  expect(screen.getByTestId("reorder-placeholder")).toHaveStyle({
    height: "50px",
    left: "171px",
    top: "28px",
    width: "100px",
  });
  expect(screen.getByTestId("reorder-ghost")).toHaveStyle({
    height: "50px",
    left: "25px",
    top: "28px",
    width: "100px",
  });

  rerender(
    <SheetBarReorderOverlay
      {...props({
        representation: {
          ghost: { sheetId: "sheet-2" },
          order: ["sheet-1", "sheet-2"],
          placeholderIndex: null,
        },
        status: "invalid",
      })}
    />,
  );
  expect(screen.getByRole("status")).toHaveTextContent(
    "Posição inválida: Páginas únicas permanecem nas extremidades.",
  );
  expect(screen.queryByTestId("reorder-placeholder")).not.toBeInTheDocument();
});

test("renders the exact double and single-page Sheet visual inside the Bar ghost", () => {
  const doubleSheet = {
    ...sheet("sheet-double", 3),
    base: {
      rgb: "#F2E4C8",
      drawRect: { x: 0, y: 0, width: 200_000, height: 100_000 },
    },
    backgrounds: [
      {
        kind: "color" as const,
        rgb: "#C9D8EC",
        drawRect: { x: 0, y: 0, width: 200_000, height: 100_000 },
      },
    ],
  } satisfies ComposedSheet;
  const singleSheet = {
    ...sheet("sheet-single", 4),
    activeSides: "right" as const,
    widthUm: 100_000,
    base: {
      rgb: "#E8D7CE",
      drawRect: { x: 0, y: 0, width: 100_000, height: 100_000 },
    },
  } satisfies ComposedSheet;
  const visualSheets = [doubleSheet, singleSheet];
  const visualLayout = createContinuousCanvasLayout(visualSheets);
  const visualMetadata = [
    {
      layoutLocked: false,
      pageNumbers: [5, 6],
      sheetId: doubleSheet.sheetId,
    },
    {
      layoutLocked: false,
      pageNumbers: [7],
      sheetId: singleSheet.sheetId,
    },
  ];
  const view = render(
    <SheetBarReorderOverlay
      {...props({
        focusedSheetId: doubleSheet.sheetId,
        layout: visualLayout,
        representation: {
          ghost: { sheetId: doubleSheet.sheetId },
          order: visualSheets.map((item) => item.sheetId),
          placeholderIndex: 1,
        },
        sheetBarMetadata: visualMetadata,
        sheets: visualSheets,
        status: "preview",
      })}
    />,
  );

  let ghost = screen.getByTestId("reorder-ghost");
  let preview = ghost.querySelector(".sheet-preview");
  expect(ghost).toHaveAttribute("data-active-sides", "both");
  expect(ghost).toHaveAttribute("data-origin-selected", "true");
  expect(preview).not.toBeNull();
  expect(preview).toHaveAttribute("viewBox", "0 0 200000 100000");
  expect(
    preview?.querySelector('[data-preview-background-color="#C9D8EC"]'),
  ).not.toBeNull();
  expect(preview?.querySelector("line")).not.toBeNull();
  expect(ghost.querySelector('[data-page-side="left"]')).toHaveTextContent(
    "5",
  );
  expect(ghost.querySelector('[data-page-side="right"]')).toHaveTextContent(
    "6",
  );

  view.rerender(
    <SheetBarReorderOverlay
      {...props({
        focusedSheetId: doubleSheet.sheetId,
        layout: visualLayout,
        representation: {
          ghost: { sheetId: singleSheet.sheetId },
          order: visualSheets.map((item) => item.sheetId),
          placeholderIndex: 0,
        },
        sheetBarMetadata: visualMetadata,
        sheets: visualSheets,
        status: "preview",
      })}
    />,
  );
  ghost = screen.getByTestId("reorder-ghost");
  preview = ghost.querySelector(".sheet-preview");
  expect(ghost).toHaveAttribute("data-active-sides", "right");
  expect(ghost).not.toHaveAttribute("data-origin-selected");
  expect(preview).toHaveAttribute("viewBox", "0 0 100000 100000");
  expect(preview?.querySelector("line")).toBeNull();
  expect(ghost.querySelector('[data-page-side="right"]')).toHaveTextContent(
    "7",
  );
  expect(
    ghost
      .querySelector<HTMLElement>(".sheet-preview-shell")
      ?.style.getPropertyValue("--sheet-inactive-side-gradient"),
  ).toContain("linear-gradient");
});

test("matches full-size markers to the bleed-cropped visible Sheet bounds", () => {
  render(
    <SheetBarReorderOverlay
      {...props({
        bleedUm: 3_000,
        representation: {
          ghost: { sheetId: "sheet-2" },
          order: ["sheet-2", "sheet-1"],
          placeholderIndex: 1,
        },
        status: "preview",
      })}
    />,
  );

  expect(barHandle(1)).toHaveStyle({ top: "29.5px" });
  expect(screen.getByTestId("reorder-placeholder")).toHaveStyle({
    height: "47px",
    top: "29.5px",
  });
  expect(screen.getByTestId("reorder-ghost")).toHaveStyle({
    height: "47px",
    top: "29.5px",
  });
});

test("keeps click and context-menu gestures distinct from reorder", () => {
  const onContextMenu = vi.fn();
  const onNavigate = vi.fn();
  const onPreview = vi.fn();
  render(
    <SheetBarReorderOverlay
      {...props({ onContextMenu, onNavigate, onPreview })}
    />,
  );

  fireEvent.contextMenu(barHandle(2), { clientX: 80, clientY: 120 });
  expect(onContextMenu).toHaveBeenCalledWith("sheet-2", { x: 80, y: 120 });
  fireEvent.click(barHandle(2));
  expect(onNavigate).toHaveBeenCalledWith("sheet-2");
  expect(onPreview).not.toHaveBeenCalled();
});

test("captures the pointer after press, crosses the threshold, and follows it in both directions", () => {
  const onDrop = vi.fn();
  const onNavigate = vi.fn();
  const onPreview = vi.fn();
  render(
    <SheetBarReorderOverlay
      {...props({ onDrop, onNavigate, onPreview })}
    />,
  );
  setOverlayBounds();
  const surface = barSurface();
  const first = barHandle(1);
  const second = barHandle(2);
  const capture = pointerCapture(surface);

  pointerDown(first, 17, 100, 40);
  expect(capture.set).toHaveBeenCalledWith(17);
  pointerMove(surface, 17, 103, 40);
  expect(onPreview).not.toHaveBeenCalled();
  expect(screen.queryByTestId("reorder-ghost")).not.toBeInTheDocument();

  pointerMove(surface, 17, 240, 40);
  expect(onPreview).toHaveBeenLastCalledWith("sheet-1", 1);
  expect(screen.getByTestId("reorder-ghost")).toHaveStyle({
    left: "165px",
    top: "28px",
  });
  expect(screen.getByTestId("reorder-ghost")).toHaveAttribute(
    "data-pointer-x",
    "240",
  );
  pointerMove(surface, 17, 260, 90);
  expect(screen.getByTestId("reorder-ghost")).toHaveStyle({
    left: "185px",
    top: "28px",
  });
  expect(screen.getByTestId("reorder-ghost")).toHaveAttribute(
    "data-pointer-y",
    "90",
  );
  pointerUp(surface, 17, 260, 90);
  expect(capture.release).toHaveBeenCalledWith(17);
  expect(onDrop).toHaveBeenCalledOnce();
  fireEvent.click(first);
  expect(onNavigate).not.toHaveBeenCalled();

  pointerDown(second, 18, 240, 40);
  pointerMove(surface, 18, 50, 40);
  expect(onPreview).toHaveBeenLastCalledWith("sheet-2", 0);
  pointerUp(surface, 18, 50, 40);
  expect(capture.release).toHaveBeenCalledWith(18);
  expect(onDrop).toHaveBeenCalledTimes(2);
});

test("keeps a below-threshold press as an ordinary Sheet click", () => {
  const onDrop = vi.fn();
  const onNavigate = vi.fn();
  const onPreview = vi.fn();
  render(
    <SheetBarReorderOverlay
      {...props({ onDrop, onNavigate, onPreview })}
    />,
  );
  const surface = barSurface();
  const first = barHandle(1);
  pointerCapture(surface);
  pointerDown(first, 9, 100, 40);
  pointerMove(surface, 9, 103, 40);
  pointerUp(surface, 9, 103, 40);
  expect(onNavigate).toHaveBeenCalledWith("sheet-1");
  fireEvent.click(first);

  expect(onNavigate).toHaveBeenCalledOnce();
  expect(onPreview).not.toHaveBeenCalled();
  expect(onDrop).not.toHaveBeenCalled();
});

test("expires synthetic click suppression before a later deliberate click", () => {
  vi.useFakeTimers();
  const onNavigate = vi.fn();
  const view = render(
    <SheetBarReorderOverlay {...props({ onNavigate })} />,
  );
  try {
    const surface = barSurface();
    const first = barHandle(1);
    pointerCapture(surface);
    pointerDown(first, 19, 100, 40);
    pointerUp(surface, 19, 100, 40);
    expect(onNavigate).toHaveBeenCalledOnce();

    vi.runOnlyPendingTimers();
    fireEvent.click(first);
    expect(onNavigate).toHaveBeenCalledTimes(2);
  } finally {
    view.unmount();
    vi.useRealTimers();
  }
});

test.each(["Escape", "pointercancel", "outside release"] as const)(
  "cancels an active Bar pointer reorder once on %s",
  (termination) => {
    const onAutoScrollVelocity = vi.fn();
    const onCancel = vi.fn();
    const onDrop = vi.fn();
    render(
      <SheetBarReorderOverlay
        {...props({ onAutoScrollVelocity, onCancel, onDrop })}
      />,
    );
    setOverlayBounds();
    const surface = barSurface();
    const first = barHandle(1);
    pointerCapture(surface);
    pointerDown(first, 31, 100, 40);
    pointerMove(surface, 31, 240, 40);

    if (termination === "Escape") {
      fireEvent.keyDown(window, { key: "Escape" });
    } else if (termination === "pointercancel") {
      fireEvent.pointerCancel(surface, { pointerId: 31 });
    } else {
      pointerUp(surface, 31, 700, 40);
    }

    expect(onCancel).toHaveBeenCalledOnce();
    expect(onDrop).not.toHaveBeenCalled();
    expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(0);
    expect(screen.queryByTestId("reorder-ghost")).not.toBeInTheDocument();
  },
);

test("reports progressive horizontal auto-scroll from captured pointer coordinates", () => {
  const onAutoScrollVelocity = vi.fn();
  render(
    <SheetBarReorderOverlay
      {...props({ onAutoScrollVelocity })}
    />,
  );
  setOverlayBounds();
  const surface = barSurface();
  const first = barHandle(1);
  pointerCapture(surface);
  pointerDown(first, 41, 100, 40);

  pointerMove(surface, 41, 36, 40);
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(-240);
  pointerMove(surface, 41, 320, 40);
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(0);
  pointerMove(surface, 41, 800, 40);
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(960);
  fireEvent.pointerCancel(surface, { pointerId: 41 });
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(0);
});

test("refreshes the Bar destination while viewport auto-scroll advances", () => {
  const onPreview = vi.fn();
  const view = render(
    <SheetBarReorderOverlay {...props({ onPreview })} />,
  );
  setOverlayBounds();
  const surface = barSurface();
  const first = barHandle(1);
  pointerCapture(surface);
  pointerDown(first, 43, 100, 40);
  pointerMove(surface, 43, 240, 40);
  expect(onPreview).toHaveBeenLastCalledWith("sheet-1", 1);

  onPreview.mockClear();
  view.rerender(
    <SheetBarReorderOverlay
      {...props({ onPreview, viewport: { offsetX: 200 } })}
    />,
  );
  expect(onPreview).toHaveBeenLastCalledWith("sheet-1", 0);
});

test("cancels and releases an active Bar reorder when its surface unmounts", () => {
  const onAutoScrollVelocity = vi.fn();
  const onCancel = vi.fn();
  const view = render(
    <SheetBarReorderOverlay
      {...props({ onAutoScrollVelocity, onCancel })}
    />,
  );
  setOverlayBounds();
  const surface = barSurface();
  const capture = pointerCapture(surface);
  pointerDown(barHandle(1), 45, 100, 40);
  pointerMove(surface, 45, 240, 40);

  view.unmount();

  expect(capture.release).toHaveBeenCalledWith(45);
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(0);
  expect(onCancel).toHaveBeenCalledOnce();
});

function barHandle(number: number): HTMLButtonElement {
  return screen.getByRole("button", {
    name: `Reordenar Lâmina ${String(number).padStart(2, "0")} pela Barra`,
  });
}

function barSurface(): HTMLElement {
  return screen.getByRole("group", {
    name: "Reordenação pela Barra da Lâmina",
  });
}

function expectPointerCoordinateInsideHandle(
  handle: HTMLElement,
  clientX: number,
  clientY: number,
) {
  const left = Number.parseFloat(handle.style.left);
  const top = Number.parseFloat(handle.style.top);
  const width = Number.parseFloat(handle.style.width);
  const height = Number.parseFloat(handle.style.height);
  expect(clientX).toBeGreaterThanOrEqual(left);
  expect(clientX).toBeLessThanOrEqual(left + width);
  expect(clientY).toBeGreaterThanOrEqual(top);
  expect(clientY).toBeLessThanOrEqual(top + height);
}

function pointerCapture(element: HTMLElement) {
  const set = vi.fn();
  const release = vi.fn();
  Object.defineProperties(element, {
    releasePointerCapture: { configurable: true, value: release },
    setPointerCapture: { configurable: true, value: set },
  });
  return { release, set };
}

function pointerDown(
  target: HTMLElement,
  pointerId: number,
  clientX: number,
  clientY: number,
) {
  fireEvent.pointerDown(target, {
    button: 0,
    buttons: 1,
    clientX,
    clientY,
    pointerId,
    pointerType: "mouse",
  });
}

function pointerMove(
  target: HTMLElement,
  pointerId: number,
  clientX: number,
  clientY: number,
) {
  fireEvent.pointerMove(target, {
    buttons: 1,
    clientX,
    clientY,
    pointerId,
    pointerType: "mouse",
  });
}

function pointerUp(
  target: HTMLElement,
  pointerId: number,
  clientX: number,
  clientY: number,
) {
  fireEvent.pointerUp(target, {
    button: 0,
    buttons: 0,
    clientX,
    clientY,
    pointerId,
    pointerType: "mouse",
  });
}

function setOverlayBounds() {
  vi.spyOn(
    screen.getByRole("group", {
      name: "Reordenação pela Barra da Lâmina",
    }),
    "getBoundingClientRect",
  ).mockReturnValue(rect(0, 0, 640, 200));
}

function rect(left: number, top: number, right: number, bottom: number): DOMRect {
  return {
    bottom,
    height: bottom - top,
    left,
    right,
    top,
    width: right - left,
    x: left,
    y: top,
    toJSON: () => ({}),
  };
}

function props(
  overrides: Partial<ComponentProps<typeof SheetBarReorderOverlay>> = {},
): ComponentProps<typeof SheetBarReorderOverlay> {
  return {
    disabled: false,
    layout,
    metrics: { scale: 0.5, width: 640 },
    onAutoScrollVelocity: vi.fn(),
    onCancel: vi.fn(),
    onContextMenu: vi.fn(),
    onDrop: vi.fn(),
    onNavigate: vi.fn(),
    onPreview: vi.fn(),
    representation: {
      ghost: null,
      order: sheets.map((item) => item.sheetId),
      placeholderIndex: null,
    },
    sheets,
    status: "idle",
    viewport: { offsetX: 25 },
    ...overrides,
  };
}

function sheet(sheetId: string, number: number): ComposedSheet {
  return {
    sheetId,
    number,
    activeSides: "both",
    widthUm: 200_000,
    heightUm: 100_000,
    base: {
      rgb: "#FFFFFF",
      drawRect: { x: 0, y: 0, width: 200_000, height: 100_000 },
    },
    backgrounds: [],
    frames: [],
    overlays: [],
  };
}
