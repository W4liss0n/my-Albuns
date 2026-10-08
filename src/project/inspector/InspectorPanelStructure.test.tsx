import { rasterLimitsAt300Dpi } from "../../test/projectConfigurationFixtures";
import { act, fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import { createTwoSheetProjection } from "../../test/projectFixtures";
import { InspectorPanel } from "./InspectorPanel";

const projection = createTwoSheetProjection();

test("opens the Sheet context menu with the explicit Grade target", () => {
  const onOpenSheetContextMenu = vi.fn();
  render(
    <InspectorPanel
      {...props()}
      onOpenSheetContextMenu={onOpenSheetContextMenu}
    />,
  );
  const firstId = projection.state.album.sheets[0].id;

  fireEvent.contextMenu(sheetSlot(firstId), {
    clientX: 80,
    clientY: 120,
  });

  expect(onOpenSheetContextMenu).toHaveBeenCalledWith(firstId, {
    x: 80,
    y: 120,
  });
});

test("captures pointer reorder in the Grade and follows it in both directions", () => {
  const onDrop = vi.fn();
  const onNavigateToSheet = vi.fn();
  const onPreview = vi.fn();
  const onSelectSheet = vi.fn();
  const ids = sheetIds();
  renderInspector({ onDrop, onNavigateToSheet, onPreview, onSelectSheet });
  const { first, grid, second } = arrangeGridBounds();
  const gridCapture = pointerCapture(grid);

  pointerDown(first, 23, 50, 50);
  expect(gridCapture.set).toHaveBeenCalledWith(23);
  pointerMove(grid, 23, 52, 52);
  expect(onPreview).not.toHaveBeenCalled();
  expect(screen.queryByTestId("reorder-ghost")).not.toBeInTheDocument();

  pointerMove(grid, 23, 50, 160);
  expect(onPreview).toHaveBeenLastCalledWith(ids[0], 1);
  expect(screen.getByTestId("reorder-ghost")).toHaveStyle({
    height: "100px",
    left: "0px",
    top: "110px",
    width: "100px",
  });
  expect(screen.getByTestId("reorder-ghost")).toHaveAttribute(
    "data-pointer-y",
    "160",
  );
  expect(screen.getByTestId("reorder-ghost")).toHaveAttribute(
    "data-active-sides",
    projection.composition.sheets[0].activeSides,
  );
  expect(screen.getByTestId("reorder-ghost")).toHaveAttribute(
    "data-origin-selected",
    "true",
  );
  expect(
    screen.getByTestId("reorder-ghost").querySelector(".sheet-preview"),
  ).not.toBeNull();
  pointerUp(grid, 23, 50, 160);
  expect(gridCapture.release).toHaveBeenCalledWith(23);
  expect(onDrop).toHaveBeenCalledOnce();
  fireEvent.click(sheetButton(1));
  fireEvent.doubleClick(sheetButton(1));
  expect(onNavigateToSheet).not.toHaveBeenCalled();
  expect(onSelectSheet).not.toHaveBeenCalled();

  pointerDown(second, 24, 50, 160);
  pointerMove(grid, 24, 50, 50);
  expect(onPreview).toHaveBeenLastCalledWith(ids[1], 0);
  pointerUp(grid, 24, 50, 50);
  expect(gridCapture.release).toHaveBeenCalledWith(24);
  expect(onDrop).toHaveBeenCalledTimes(2);
});

test("removes the Grade ghost on release while the reorder commits", () => {
  const ids = sheetIds();
  const panelProps = props();
  const sheetReorder = (status: "committing" | "preview") => ({
    disabled: false,
    onCancel: vi.fn(),
    onDrop: vi.fn(),
    onPreview: vi.fn(),
    representation: {
      ghost: { sheetId: ids[0] },
      order: ids,
      placeholderIndex: 1,
    },
    status,
  });
  const view = render(
    <InspectorPanel {...panelProps} sheetReorder={sheetReorder("preview")} />,
  );
  const { grid } = dragFirstGridSlot(62);
  expect(screen.getByTestId("reorder-ghost")).toBeInTheDocument();

  pointerUp(grid, 62, 50, 160);
  view.rerender(
    <InspectorPanel
      {...panelProps}
      sheetReorder={sheetReorder("committing")}
    />,
  );

  expect(screen.queryByTestId("reorder-ghost")).not.toBeInTheDocument();
});

test("keeps the exact single-page source representation in the Grade ghost", () => {
  const firstSheet = projection.composition.sheets[0];
  const singlePageSheet = {
    ...firstSheet,
    activeSides: "right" as const,
    widthUm: firstSheet.widthUm / 2,
  };
  const panelProps = props();
  render(
    <InspectorPanel
      {...panelProps}
      sheets={[singlePageSheet, projection.composition.sheets[1]]}
      sheetStates={[
        {
          ...projection.state.album.sheets[0],
          pageNumbers: [1],
          role: "initial",
        },
        projection.state.album.sheets[1],
      ]}
      sheetReorder={{
        disabled: false,
        onCancel: vi.fn(),
        onDrop: vi.fn(),
        onPreview: vi.fn(),
        representation: {
          ghost: { sheetId: singlePageSheet.sheetId },
          order: sheetIds(),
          placeholderIndex: 1,
        },
        status: "preview",
      }}
    />,
  );
  dragFirstGridSlot();

  const sourcePreview = sheetButton(1).querySelector(".sheet-preview");
  const ghost = screen.getByTestId("reorder-ghost");
  const ghostShell = ghost.querySelector<HTMLElement>(
    ".sheet-preview-shell",
  );
  const ghostPreview = ghost.querySelector(".sheet-preview");
  expect(ghost).toHaveAttribute("data-active-sides", "right");
  expect(ghostPreview).toHaveAttribute(
    "viewBox",
    sourcePreview?.getAttribute("viewBox"),
  );
  expect(
    ghostShell?.style.getPropertyValue("--sheet-inactive-side-gradient"),
  ).toContain("linear-gradient");
});

test("selects on a below-threshold Grade press without navigating", () => {
  const onDrop = vi.fn();
  const onNavigateToSheet = vi.fn();
  const onPreview = vi.fn();
  const onSelectSheet = vi.fn();
  renderInspector({ onDrop, onNavigateToSheet, onPreview, onSelectSheet });
  const { first, grid } = arrangeGridBounds();
  pointerCapture(grid);

  pointerDown(first, 12, 50, 50);
  pointerMove(grid, 12, 52, 52);
  pointerUp(grid, 12, 52, 52);
  expect(onSelectSheet).toHaveBeenCalledWith(sheetIds()[0], {
    range: false,
    toggle: false,
  });
  fireEvent.click(sheetButton(1));

  expect(onSelectSheet).toHaveBeenCalledOnce();
  expect(onNavigateToSheet).not.toHaveBeenCalled();
  expect(onPreview).not.toHaveBeenCalled();
  expect(onDrop).not.toHaveBeenCalled();
});

test("passes Ctrl, Cmd and Shift from a Grade press to the selection", () => {
  const onSelectSheet = vi.fn();
  renderInspector({ onSelectSheet });
  const { first, grid, second } = arrangeGridBounds();
  pointerCapture(grid);

  pointerDown(second, 31, 50, 160);
  pointerUp(grid, 31, 50, 160, { ctrlKey: true });
  pointerDown(first, 32, 50, 50);
  pointerUp(grid, 32, 50, 50, { metaKey: true });
  pointerDown(second, 33, 50, 160);
  pointerUp(grid, 33, 50, 160, { shiftKey: true });
  pointerDown(first, 34, 50, 50);
  pointerUp(grid, 34, 50, 50, { ctrlKey: true, shiftKey: true });

  const [firstId, secondId] = sheetIds();
  expect(onSelectSheet.mock.calls).toEqual([
    [secondId, { range: false, toggle: true }],
    [firstId, { range: false, toggle: true }],
    [secondId, { range: true, toggle: false }],
    [firstId, { range: true, toggle: true }],
  ]);
});

test("selects with a click when the Grade cannot reorder", () => {
  const onNavigateToSheet = vi.fn();
  const onSelectSheet = vi.fn();
  render(
    <InspectorPanel
      {...props()}
      onNavigateToSheet={onNavigateToSheet}
      onSelectSheet={onSelectSheet}
    />,
  );

  fireEvent.click(sheetButton(2), { ctrlKey: true });
  fireEvent.click(sheetButton(1), { shiftKey: true });

  expect(onSelectSheet.mock.calls).toEqual([
    [sheetIds()[1], { range: false, toggle: true }],
    [sheetIds()[0], { range: true, toggle: false }],
  ]);
  expect(onNavigateToSheet).not.toHaveBeenCalled();
});

test("goes to a Sheet only on a double click in the Grade", () => {
  const onNavigateToSheet = vi.fn();
  renderInspector({ onNavigateToSheet });
  const { grid } = arrangeGridBounds();

  fireEvent.doubleClick(sheetButton(2));
  expect(onNavigateToSheet).toHaveBeenLastCalledWith(sheetIds()[1]);

  // Under pointer capture the double click reaches the Grade itself.
  fireEvent.doubleClick(grid, { clientX: 50, clientY: 50 });
  expect(onNavigateToSheet).toHaveBeenLastCalledWith(sheetIds()[0]);

  fireEvent.doubleClick(grid, { clientX: 50, clientY: 105 });
  expect(onNavigateToSheet).toHaveBeenCalledTimes(2);
});

test("does not go to a Sheet when the second press of a double click was a drag", () => {
  const onNavigateToSheet = vi.fn();
  renderInspector({ onNavigateToSheet });
  const { first, grid } = arrangeGridBounds();
  pointerCapture(grid);

  pointerDown(first, 41, 50, 50);
  pointerMove(grid, 41, 50, 160);
  pointerUp(grid, 41, 50, 160);
  fireEvent.doubleClick(grid, { clientX: 50, clientY: 160 });
  expect(onNavigateToSheet).not.toHaveBeenCalled();

  pointerDown(first, 42, 50, 50);
  pointerUp(grid, 42, 50, 50);
  fireEvent.doubleClick(sheetButton(1));
  expect(onNavigateToSheet).toHaveBeenCalledWith(sheetIds()[0]);
});

test("does not go to a Sheet when a fast drag and its release share one frame", () => {
  const onDrop = vi.fn();
  const onNavigateToSheet = vi.fn();
  renderInspector({ onDrop, onNavigateToSheet });
  const { first, grid } = arrangeGridBounds();
  pointerCapture(grid);

  pointerDown(first, 51, 50, 50);
  act(() => {
    pointerMove(grid, 51, 50, 160);
    pointerUp(grid, 51, 50, 160);
  });
  fireEvent.doubleClick(grid, { clientX: 50, clientY: 160 });

  expect(onDrop).toHaveBeenCalledOnce();
  expect(onNavigateToSheet).not.toHaveBeenCalled();
});

test("a drag cancelled with Escape keeps the selection when the button is released", async () => {
  const onCancel = vi.fn();
  const onNavigateToSheet = vi.fn();
  const onSelectSheet = vi.fn();
  renderInspector({ onCancel, onNavigateToSheet, onSelectSheet });
  const { first, grid } = arrangeGridBounds();
  pointerCapture(grid);

  pointerDown(first, 61, 50, 50);
  pointerMove(grid, 61, 50, 160);
  fireEvent.keyDown(window, { key: "Escape" });
  expect(onCancel).toHaveBeenCalledOnce();
  // The user takes a while to let go of the button.
  await act(() => new Promise((resolve) => setTimeout(resolve, 0)));

  pointerUp(sheetButton(1), 61, 50, 50);
  fireEvent.click(sheetButton(1));
  fireEvent.doubleClick(sheetButton(1));
  expect(onSelectSheet).not.toHaveBeenCalled();
  expect(onNavigateToSheet).not.toHaveBeenCalled();

  await act(() => new Promise((resolve) => setTimeout(resolve, 0)));
  fireEvent.click(sheetButton(1));
  expect(onSelectSheet).toHaveBeenCalledWith(sheetIds()[0], {
    range: false,
    toggle: false,
  });
});

test("Enter goes to the Sheet and Space selects it from the keyboard", () => {
  const onNavigateToSheet = vi.fn();
  const onSelectSheet = vi.fn();
  renderInspector({ onNavigateToSheet, onSelectSheet });

  fireEvent.keyDown(sheetButton(2), { key: "Enter" });
  expect(onNavigateToSheet).toHaveBeenCalledWith(sheetIds()[1]);
  expect(onSelectSheet).not.toHaveBeenCalled();

  fireEvent.keyDown(sheetButton(1), { key: " " });
  fireEvent.keyDown(sheetButton(2), { ctrlKey: true, key: " " });
  // A held key is swallowed: the button must not replay it as its own click.
  expect(
    fireEvent.keyDown(sheetButton(2), { ctrlKey: true, key: " ", repeat: true }),
  ).toBe(false);
  expect(
    fireEvent.keyDown(sheetButton(2), { key: "Enter", repeat: true }),
  ).toBe(false);
  expect(onSelectSheet.mock.calls).toEqual([
    [sheetIds()[0], { range: false, toggle: false }],
    [sheetIds()[1], { range: false, toggle: true }],
  ]);

  // Ctrl+Enter stays with the application shortcut (Adicionar depois).
  const notPrevented = fireEvent.keyDown(sheetButton(1), {
    ctrlKey: true,
    key: "Enter",
  });
  expect(notPrevented).toBe(true);
  expect(onNavigateToSheet).toHaveBeenCalledOnce();
});

test("shows every selected Sheet and keeps the focused one current", () => {
  const [firstId, secondId] = sheetIds();
  render(
    <InspectorPanel
      {...props()}
      focusedSheetId={secondId}
      selectedSheetIds={[firstId, secondId]}
      sheetReorder={{
        disabled: false,
        onCancel: vi.fn(),
        onDrop: vi.fn(),
        onPreview: vi.fn(),
        representation: {
          ghost: { sheetId: firstId },
          order: sheetIds(),
          placeholderIndex: 0,
        },
        status: "preview",
      }}
    />,
  );
  dragFirstGridSlot();

  expect(sheetButton(1)).toHaveAttribute("aria-pressed", "true");
  expect(sheetButton(2)).toHaveAttribute("aria-pressed", "true");
  expect(sheetButton(1)).toHaveClass("active");
  expect(sheetButton(2)).toHaveClass("active");
  expect(sheetButton(1)).not.toHaveAttribute("aria-current");
  expect(sheetButton(2)).toHaveAttribute("aria-current", "true");
  expect(sheetButton(1)).toHaveAccessibleDescription(
    "Duplo clique para ir até a lâmina",
  );
  expect(screen.getByTestId("reorder-ghost")).toHaveAttribute(
    "data-origin-selected",
    "true",
  );
});

test("keeps Grade structural pointer gestures disabled during Sheet Edit Mode", () => {
  const onPreview = vi.fn();
  renderInspector({ disabled: true, onPreview });
  const first = sheetSlot(sheetIds()[0]);
  pointerCapture(first);

  expect(first).not.toHaveAttribute("draggable");
  expect(first).not.toHaveAttribute("data-reorder-enabled");
  pointerDown(first, 4, 50, 50);
  pointerMove(first, 4, 50, 160);
  expect(onPreview).not.toHaveBeenCalled();
});

test("announces an invalid Grade target without inventing a placeholder", () => {
  const ids = sheetIds();
  render(
    <InspectorPanel
      {...props()}
      sheetReorder={{
        disabled: false,
        onCancel: vi.fn(),
        onDrop: vi.fn(),
        onPreview: vi.fn(),
        representation: {
          ghost: { sheetId: ids[0] },
          order: ids,
          placeholderIndex: null,
        },
        status: "invalid",
      }}
    />,
  );
  dragFirstGridSlot();

  expect(
    screen.getByText(
      "Posição inválida: páginas únicas permanecem nas extremidades.",
    ),
  ).toHaveAttribute("role", "status");
  expect(screen.queryByTestId("reorder-placeholder")).not.toBeInTheDocument();
  expect(screen.getByTestId("reorder-ghost")).toBeInTheDocument();
});

test.each(["Escape", "pointercancel", "outside release"] as const)(
  "cancels an active Grade pointer reorder once on %s",
  (termination) => {
    const onCancel = vi.fn();
    const onDrop = vi.fn();
    renderInspector({ onCancel, onDrop });
    const { first } = arrangeGridBounds();
    pointerCapture(first);
    pointerDown(first, 33, 50, 50);
    pointerMove(first, 33, 50, 160);

    if (termination === "Escape") {
      fireEvent.keyDown(window, { key: "Escape" });
    } else if (termination === "pointercancel") {
      fireEvent.pointerCancel(first, { pointerId: 33 });
    } else {
      pointerUp(first, 33, 50, 500);
    }

    expect(onCancel).toHaveBeenCalledOnce();
    expect(onDrop).not.toHaveBeenCalled();
    expect(screen.queryByTestId("reorder-ghost")).not.toBeInTheDocument();
  },
);

test("keeps progressive Grade auto-scroll running between pointer moves", () => {
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrameId = 1;
  const requestFrame = vi
    .spyOn(window, "requestAnimationFrame")
    .mockImplementation((callback) => {
      const frameId = nextFrameId++;
      frames.set(frameId, callback);
      return frameId;
    });
  const cancelFrame = vi
    .spyOn(window, "cancelAnimationFrame")
    .mockImplementation((frameId) => {
      frames.delete(frameId);
    });
  const onCancel = vi.fn();
  const view = renderInspector({ onCancel });
  try {
    const { first, grid, viewport } = arrangeGridBounds({
      gridBottom: 800,
      followScroll: true,
      viewportBottom: 400,
    });
    viewport.scrollTop = 100;
    pointerCapture(first);
    pointerDown(first, 44, 50, 50);
    pointerMove(first, 44, 50, 450);
    expect(requestFrame).toHaveBeenCalledOnce();

    runNextFrame(frames, 1_000);
    expect(viewport.scrollTop).toBe(100);
    runNextFrame(frames, 1_016);
    expect(viewport.scrollTop).toBeCloseTo(111.52);
    runNextFrame(frames, 1_032);
    expect(viewport.scrollTop).toBeCloseTo(123.04);

    pointerMove(first, 44, 50, 364);
    runNextFrame(frames, 1_048);
    expect(viewport.scrollTop).toBeCloseTo(125.92);

    const pendingFrameId = [...frames.keys()][0];
    fireEvent.pointerCancel(first, { pointerId: 44 });
    expect(cancelFrame).toHaveBeenLastCalledWith(pendingFrameId);
    expect(frames.size).toBe(0);
    expect(onCancel).toHaveBeenCalledOnce();
    expect(grid).toBeInTheDocument();
  } finally {
    view.unmount();
    vi.restoreAllMocks();
  }
});

test("refreshes the Grade destination while auto-scroll advances", () => {
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrameId = 1;
  vi.spyOn(window, "requestAnimationFrame").mockImplementation((callback) => {
    const frameId = nextFrameId++;
    frames.set(frameId, callback);
    return frameId;
  });
  vi.spyOn(window, "cancelAnimationFrame").mockImplementation((frameId) => {
    frames.delete(frameId);
  });
  const onPreview = vi.fn();
  const view = renderInspector({ onPreview });
  try {
    const { first, grid, viewport } = arrangeGridBounds({
      followScroll: true,
      gridBottom: 800,
      viewportBottom: 100,
    });
    pointerCapture(grid);
    pointerDown(first, 46, 50, 50);
    pointerMove(grid, 46, 50, 95);
    expect(onPreview).toHaveBeenLastCalledWith(sheetIds()[0], 0);

    runNextFrame(frames, 1_000);
    onPreview.mockClear();
    runNextFrame(frames, 1_050);

    expect(viewport.scrollTop).toBeGreaterThan(0);
    expect(onPreview).toHaveBeenLastCalledWith(sheetIds()[0], 1);
  } finally {
    view.unmount();
    vi.restoreAllMocks();
  }
});

test.each(["pointercancel", "Escape", "unmount"] as const)(
  "stops the Grade auto-scroll frame on %s",
  (termination) => {
    const frames = new Map<number, FrameRequestCallback>();
    const requestFrame = vi
      .spyOn(window, "requestAnimationFrame")
      .mockImplementation((callback) => {
        frames.set(41, callback);
        return 41;
      });
    const cancelFrame = vi
      .spyOn(window, "cancelAnimationFrame")
      .mockImplementation((frameId) => {
        frames.delete(frameId);
      });
    const onCancel = vi.fn();
    const view = renderInspector({ onCancel });
    let mounted = true;
    try {
      const { first } = arrangeGridBounds({
        gridBottom: 800,
        viewportBottom: 400,
      });
      pointerCapture(first);
      pointerDown(first, 51, 50, 50);
      pointerMove(first, 51, 50, 450);
      expect(requestFrame).toHaveBeenCalledOnce();

      if (termination === "pointercancel") {
        fireEvent.pointerCancel(first, { pointerId: 51 });
      } else if (termination === "Escape") {
        fireEvent.keyDown(window, { key: "Escape" });
      } else {
        view.unmount();
        mounted = false;
      }

      expect(cancelFrame).toHaveBeenCalledWith(41);
      expect(frames.size).toBe(0);
      expect(onCancel).toHaveBeenCalledOnce();
    } finally {
      if (mounted) view.unmount();
      vi.restoreAllMocks();
    }
  },
);

function renderInspector({
  disabled = false,
  onCancel = vi.fn(),
  onDrop = vi.fn(),
  onNavigateToSheet = vi.fn(),
  onPreview = vi.fn(),
  onSelectSheet = vi.fn(),
}: {
  disabled?: boolean;
  onCancel?: NonNullable<
    ComponentProps<typeof InspectorPanel>["sheetReorder"]
  >["onCancel"];
  onDrop?: NonNullable<
    ComponentProps<typeof InspectorPanel>["sheetReorder"]
  >["onDrop"];
  onNavigateToSheet?: ComponentProps<
    typeof InspectorPanel
  >["onNavigateToSheet"];
  onPreview?: NonNullable<
    ComponentProps<typeof InspectorPanel>["sheetReorder"]
  >["onPreview"];
  onSelectSheet?: ComponentProps<typeof InspectorPanel>["onSelectSheet"];
} = {}) {
  return render(
    <InspectorPanel
      {...props()}
      onNavigateToSheet={onNavigateToSheet}
      onSelectSheet={onSelectSheet}
      sheetReorder={{
        disabled,
        onCancel,
        onDrop,
        onPreview,
        representation: {
          ghost: null,
          order: sheetIds(),
          placeholderIndex: null,
        },
        status: "idle",
      }}
    />,
  );
}

function arrangeGridBounds({
  followScroll = false,
  gridBottom = 220,
  viewportBottom = 220,
}: {
  followScroll?: boolean;
  gridBottom?: number;
  viewportBottom?: number;
} = {}) {
  const [first, second] = Array.from(
    document.querySelectorAll<HTMLElement>(".sheet-grid-slot"),
  );
  const grid = screen.getByTestId("sheet-reorder-grid");
  const viewport = document.querySelector<HTMLElement>(".inspector-scroll")!;
  const scrollFactor = Number(followScroll);
  vi.spyOn(first!, "getBoundingClientRect").mockImplementation(() =>
    rect(
      0,
      -scrollFactor * viewport.scrollTop,
      100,
      100 - scrollFactor * viewport.scrollTop,
    ),
  );
  vi.spyOn(second!, "getBoundingClientRect").mockImplementation(() =>
    rect(
      0,
      110 - scrollFactor * viewport.scrollTop,
      100,
      210 - scrollFactor * viewport.scrollTop,
    ),
  );
  vi.spyOn(grid, "getBoundingClientRect").mockReturnValue(
    rect(0, 0, 220, gridBottom),
  );
  vi.spyOn(viewport, "getBoundingClientRect").mockReturnValue(
    rect(0, 0, 220, viewportBottom),
  );
  return { first: first!, grid, second: second!, viewport };
}

/** Holds the first slot dragged down past the threshold, so its ghost floats. */
function dragFirstGridSlot(pointerId = 61) {
  const bounds = arrangeGridBounds();
  pointerCapture(bounds.grid);
  pointerDown(bounds.first, pointerId, 50, 50);
  pointerMove(bounds.grid, pointerId, 50, 160);
  return bounds;
}

function sheetIds() {
  return projection.state.album.sheets.map((sheet) => sheet.id);
}

function sheetSlot(sheetId: string): HTMLElement {
  return document.querySelector<HTMLElement>(
    `.sheet-grid-slot[data-sheet-id="${sheetId}"]`,
  )!;
}

function sheetButton(number: number): HTMLElement {
  return screen.getByRole("button", {
    name: new RegExp(`^Lâmina ${String(number).padStart(2, "0")},`),
  });
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
  modifiers: { ctrlKey?: boolean; metaKey?: boolean; shiftKey?: boolean } = {},
) {
  fireEvent.pointerUp(target, {
    button: 0,
    buttons: 0,
    clientX,
    clientY,
    pointerId,
    pointerType: "mouse",
    ...modifiers,
  });
}

function runNextFrame(
  frames: Map<number, FrameRequestCallback>,
  timestamp: number,
) {
  const next = frames.entries().next().value;
  if (!next) throw new Error("Nenhum quadro de autoscroll agendado.");
  const [frameId, callback] = next;
  frames.delete(frameId);
  callback(timestamp);
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

function props(): ComponentProps<typeof InspectorPanel> {
  return {
    context: { kind: "album" },
    displayedPhotoZoom: 1,
    document: projection.state.document,
    focusedSheetId: projection.state.album.sheets[0].id,
    mediaItems: projection.state.album.media,
    mediaPreviews: {},
    onApplyAlbumDesign: vi.fn(),
    onApplyAlbumInformation: vi.fn(),    onNavigateToSheet: vi.fn(),
    onSelectSheet: vi.fn(),
    onPresentationUnitChange: vi.fn(),    onValidateAlbumInformation: vi.fn(async () => ({ rasterLimits: rasterLimitsAt300Dpi,
      errors: [],
      impact: { conversionLosses: [], heightPx: 1, pageWidthPx: 1, sheetWidthPx: 2 },
    })),
    presentationUnit: projection.state.document.displayUnit,
    revision: projection.state.revision,
    sectionState: { kind: "local" },
    sheets: projection.composition.sheets,
    sheetStates: projection.state.album.sheets,
    visualDefaults: projection.state.album.visualDefaults,
    frameGapUm: projection.state.layoutSettings.gapUm,  };
}
