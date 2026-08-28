import {
  createEvent,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import type { ComposedSheet } from "../domain/project";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import { SheetBarReorderOverlay } from "./SheetBarReorderOverlay";

const sheets = [sheet("sheet-1", 1), sheet("sheet-2", 2)];
const layout = createContinuousCanvasLayout(sheets);

test("aligns enabled reorder handles with the scaled Sheet Bar slots", () => {
  const { rerender } = render(<SheetBarReorderOverlay {...props()} />);

  const first = screen.getByRole("button", {
    name: "Reordenar Lâmina 01 pela Barra",
  });
  const second = screen.getByRole("button", {
    name: "Reordenar Lâmina 02 pela Barra",
  });
  expect(first).toHaveStyle({ left: "25px", top: "28px", width: "100px" });
  expect(second).toHaveStyle({ left: "171px", top: "28px", width: "100px" });
  expect(first).toHaveAttribute("draggable", "true");

  rerender(<SheetBarReorderOverlay {...props()} disabled />);
  expect(first).toHaveAttribute("draggable", "false");

  rerender(
    <SheetBarReorderOverlay {...props({ status: "committing" })} />,
  );
  expect(first).toHaveAttribute("draggable", "false");
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

  const movedNeighbor = screen.getByRole("button", {
    name: "Reordenar Lâmina 01 pela Barra",
  });
  const dragged = screen.getByRole("button", {
    name: "Reordenar Lâmina 02 pela Barra",
  });
  expect(movedNeighbor).toHaveStyle({ left: "171px" });
  expect(movedNeighbor).toHaveAttribute("data-reorder-shift", "true");
  expect(dragged).toHaveStyle({ left: "25px" });
  expect(dragged).toHaveAttribute("data-reorder-ghost", "true");

  expect(screen.getByTestId("reorder-placeholder")).toHaveStyle({
    left: "171px",
    top: "28px",
    width: "100px",
  });
  expect(screen.getByTestId("reorder-ghost")).toHaveTextContent("02");

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

test("routes native drag, drop, end, and context gestures through the Bar seam", () => {
  const onPreview = vi.fn();
  const onDrop = vi.fn();
  const onCancel = vi.fn();
  const onContextMenu = vi.fn();
  const onNavigate = vi.fn();
  render(
    <SheetBarReorderOverlay
      {...props({
        onCancel,
        onContextMenu,
        onDrop,
        onNavigate,
        onPreview,
      })}
    />,
  );
  const first = screen.getByRole("button", {
    name: "Reordenar Lâmina 01 pela Barra",
  });
  const second = screen.getByRole("button", {
    name: "Reordenar Lâmina 02 pela Barra",
  });
  const dataTransfer = {
    dropEffect: "none",
    effectAllowed: "none",
    setData: vi.fn(),
  };

  fireEvent.dragStart(first, { dataTransfer });
  expect(dataTransfer.effectAllowed).toBe("move");
  expect(dataTransfer.setData).toHaveBeenCalledWith("text/plain", "sheet-1");
  expect(onPreview).toHaveBeenLastCalledWith("sheet-1", 0);

  fireEvent.dragEnter(second, { dataTransfer });
  expect(onPreview).toHaveBeenLastCalledWith("sheet-1", 1);

  fireEvent.drop(
    screen.getByTestId("sheet-reorder-bar-drop-zone"),
    { dataTransfer },
  );
  expect(onDrop).toHaveBeenCalledOnce();

  fireEvent.dragEnd(first, { dataTransfer });
  expect(onCancel).not.toHaveBeenCalled();

  fireEvent.dragStart(first, { dataTransfer });
  fireEvent.dragEnd(first, { dataTransfer });
  expect(onCancel).toHaveBeenCalledOnce();

  fireEvent.contextMenu(second, { clientX: 80, clientY: 120 });
  expect(onContextMenu).toHaveBeenCalledWith("sheet-2", { x: 80, y: 120 });
  fireEvent.click(second);
  expect(onNavigate).toHaveBeenCalledWith("sheet-2");
});

test("anchors a visible custom native drag image to the pointer", () => {
  vi.useFakeTimers();
  try {
    render(<SheetBarReorderOverlay {...props()} />);
    const first = screen.getByRole("button", {
      name: "Reordenar Lâmina 01 pela Barra",
    });
    vi.spyOn(first, "getBoundingClientRect").mockReturnValue({
      bottom: 68,
      height: 40,
      left: 20,
      right: 120,
      top: 28,
      width: 100,
      x: 20,
      y: 28,
      toJSON: () => ({}),
    });
    const setDragImage = vi.fn(
      (element: HTMLElement, offsetX: number, offsetY: number) => {
        expect(document.body).toContainElement(element);
        expect(element).toHaveClass(
          "sheet-bar-reorder-overlay__native-drag-image",
        );
        expect(element).toHaveTextContent("L01");
        expect(element).toHaveStyle({ height: "40px", width: "100px" });
        expect([offsetX, offsetY]).toEqual([50, 20]);
      },
    );
    const dataTransfer = {
      dropEffect: "none",
      effectAllowed: "none",
      setData: vi.fn(),
      setDragImage,
    };

    dragStartAt(first, 70, 48, dataTransfer);

    expect(setDragImage).toHaveBeenCalledOnce();
    const dragImage = setDragImage.mock.calls[0][0];
    vi.runOnlyPendingTimers();
    expect(document.body).not.toContainElement(dragImage);
  } finally {
    vi.useRealTimers();
  }
});

test("falls back to a visible source when the custom drag image is rejected", () => {
  vi.useFakeTimers();
  try {
    render(<SheetBarReorderOverlay {...props()} />);
    const first = screen.getByRole("button", {
      name: "Reordenar Lâmina 01 pela Barra",
    });
    vi.spyOn(first, "getBoundingClientRect").mockReturnValue({
      bottom: 68,
      height: 40,
      left: 20,
      right: 120,
      top: 28,
      width: 100,
      x: 20,
      y: 28,
      toJSON: () => ({}),
    });
    const dataTransfer = {
      dropEffect: "none",
      effectAllowed: "none",
      setData: vi.fn(),
      setDragImage: vi.fn(() => {
        throw new Error("drag image unavailable");
      }),
    };

    expect(() => dragStartAt(first, 70, 48, dataTransfer)).not.toThrow();
    expect(first).toHaveAttribute("data-native-drag-fallback", "true");
    expect(first).toHaveTextContent("L01");
    expect(
      document.querySelector(
        ".sheet-bar-reorder-overlay__native-drag-image",
      ),
    ).not.toBeInTheDocument();

    vi.runOnlyPendingTimers();
    expect(first).not.toHaveAttribute("data-native-drag-fallback");
  } finally {
    vi.useRealTimers();
  }
});

test("reports progressive horizontal automatic scrolling at the Canvas edges", () => {
  const onAutoScrollVelocity = vi.fn();
  render(
    <SheetBarReorderOverlay
      {...props({ onAutoScrollVelocity })}
    />,
  );
  const overlay = screen.getByRole("group", {
    name: "Reordenação pela Barra da Lâmina",
  });
  vi.spyOn(overlay, "getBoundingClientRect").mockReturnValue({
    bottom: 200,
    height: 200,
    left: 0,
    right: 640,
    top: 0,
    width: 640,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  const first = screen.getByRole("button", {
    name: "Reordenar Lâmina 01 pela Barra",
  });
  const dropZone = screen.getByTestId("sheet-reorder-bar-drop-zone");
  const dataTransfer = {
    dropEffect: "none",
    effectAllowed: "none",
    setData: vi.fn(),
  };
  fireEvent.dragStart(first, { dataTransfer });

  dragOverAt(dropZone, 36, dataTransfer);
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(-240);

  dragOverAt(dropZone, 320, dataTransfer);
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(0);

  dragOverAt(dropZone, 800, dataTransfer);
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(960);

  fireEvent.dragEnd(first, { dataTransfer });
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(0);
});

test("cancels an active Bar drag once when Escape ends the native gesture", () => {
  const onCancel = vi.fn();
  const onAutoScrollVelocity = vi.fn();
  render(
    <SheetBarReorderOverlay
      {...props({ onAutoScrollVelocity, onCancel })}
    />,
  );
  const first = screen.getByRole("button", {
    name: "Reordenar Lâmina 01 pela Barra",
  });
  const dataTransfer = {
    dropEffect: "none",
    effectAllowed: "none",
    setData: vi.fn(),
  };

  fireEvent.keyDown(window, { key: "Escape" });
  expect(onCancel).not.toHaveBeenCalled();

  fireEvent.dragStart(first, { dataTransfer });
  fireEvent.keyDown(window, { key: "Escape" });
  expect(onCancel).toHaveBeenCalledOnce();
  expect(onAutoScrollVelocity).toHaveBeenLastCalledWith(0);

  fireEvent.dragEnd(first, { dataTransfer });
  expect(onCancel).toHaveBeenCalledOnce();
});

function dragOverAt(
  target: HTMLElement,
  clientX: number,
  dataTransfer: {
    dropEffect: string;
    effectAllowed: string;
    setData: ReturnType<typeof vi.fn>;
  },
) {
  const event = createEvent.dragOver(target, { dataTransfer });
  Object.defineProperty(event, "clientX", { value: clientX });
  fireEvent(target, event);
}

function dragStartAt(
  target: HTMLElement,
  clientX: number,
  clientY: number,
  dataTransfer: {
    dropEffect: string;
    effectAllowed: string;
    setData: ReturnType<typeof vi.fn>;
    setDragImage: ReturnType<typeof vi.fn>;
  },
) {
  const event = createEvent.dragStart(target, { dataTransfer });
  Object.defineProperties(event, {
    clientX: { value: clientX },
    clientY: { value: clientY },
  });
  fireEvent(target, event);
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
