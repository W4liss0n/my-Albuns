import { fireEvent, render, screen } from "@testing-library/react";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import { createTwoSheetProjection } from "../test/projectFixtures";
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
  const ids = sheetIds();
  renderInspector({ onDrop, onNavigateToSheet, onPreview });
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
  pointerUp(grid, 23, 50, 160);
  expect(gridCapture.release).toHaveBeenCalledWith(23);
  expect(onDrop).toHaveBeenCalledOnce();
  fireEvent.click(sheetButton(1));
  expect(onNavigateToSheet).not.toHaveBeenCalled();

  pointerDown(second, 24, 50, 160);
  pointerMove(grid, 24, 50, 50);
  expect(onPreview).toHaveBeenLastCalledWith(ids[1], 0);
  pointerUp(grid, 24, 50, 50);
  expect(gridCapture.release).toHaveBeenCalledWith(24);
  expect(onDrop).toHaveBeenCalledTimes(2);
});

test("keeps a below-threshold Grade press as an ordinary navigation click", () => {
  const onDrop = vi.fn();
  const onNavigateToSheet = vi.fn();
  const onPreview = vi.fn();
  renderInspector({ onDrop, onNavigateToSheet, onPreview });
  const { first, grid } = arrangeGridBounds();
  pointerCapture(grid);

  pointerDown(first, 12, 50, 50);
  pointerMove(grid, 12, 52, 52);
  pointerUp(grid, 12, 52, 52);
  expect(onNavigateToSheet).toHaveBeenCalledWith(sheetIds()[0]);
  fireEvent.click(sheetButton(1));

  expect(onNavigateToSheet).toHaveBeenCalledOnce();
  expect(onPreview).not.toHaveBeenCalled();
  expect(onDrop).not.toHaveBeenCalled();
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

  expect(
    screen.getByText(
      "Posição inválida: Páginas únicas permanecem nas extremidades.",
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
} = {}) {
  return render(
    <InspectorPanel
      {...props()}
      onNavigateToSheet={onNavigateToSheet}
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
    name: new RegExp(`^Ir para Lâmina ${String(number).padStart(2, "0")},`),
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
    displayedPhotoPanX: 0,
    displayedPhotoZoom: 1,
    document: projection.state.document,
    focusedSheetId: projection.state.album.sheets[0].id,
    frameBorder: projection.composition.frameBorder,
    mediaItems: projection.state.album.media,
    mediaPreviews: {},
    onApplyAlbumDesign: vi.fn(),
    onApplyAlbumInformation: vi.fn(),
    onBeginPhotoZoom: vi.fn(),
    onFinishPhotoZoom: vi.fn(),
    onNavigateToSheet: vi.fn(),
    onPresentationUnitChange: vi.fn(),
    onUpdatePhotoZoom: vi.fn(),
    onValidateAlbumInformation: vi.fn(async () => ({
      errors: [],
      impact: { heightPx: 1, pageWidthPx: 1, sheetWidthPx: 2 },
    })),
    presentationUnit: projection.state.document.displayUnit,
    revision: projection.state.revision,
    sectionState: { kind: "local" },
    sheets: projection.composition.sheets,
    sheetStates: projection.state.album.sheets,
    visualDefaults: projection.state.album.visualDefaults,
    zoomCommitting: false,
  };
}
