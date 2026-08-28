import {
  createEvent,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import { createTwoSheetProjection } from "../test/projectFixtures";
import { InspectorPanel } from "./InspectorPanel";

const projection = createTwoSheetProjection();

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

test("opens the Sheet context menu with the explicit Grade target", () => {
  const onOpenSheetContextMenu = vi.fn();
  render(
    <InspectorPanel
      {...props()}
      onOpenSheetContextMenu={onOpenSheetContextMenu}
    />,
  );
  const firstId = projection.state.album.sheets[0].id;
  const tile = document.querySelector(`[data-sheet-id="${firstId}"]`)!;

  fireEvent.contextMenu(tile, { clientX: 80, clientY: 120 });

  expect(onOpenSheetContextMenu).toHaveBeenCalledWith(firstId, {
    x: 80,
    y: 120,
  });
});

test("previews and drops a reorder only through the productive Grade", () => {
  const onPreview = vi.fn();
  const onDrop = vi.fn();
  const onCancel = vi.fn();
  const ids = projection.state.album.sheets.map((sheet) => sheet.id);
  render(
    <InspectorPanel
      {...props()}
      sheetReorder={{
        disabled: false,
        onCancel,
        onDrop,
        onPreview,
        representation: {
          ghost: null,
          order: ids,
          placeholderIndex: null,
        },
        status: "idle",
      }}
    />,
  );
  const tiles = Array.from(document.querySelectorAll("[data-sheet-id]"));
  const dataTransfer = {
    effectAllowed: "none",
    setData: vi.fn(),
  };

  fireEvent.dragStart(tiles[0], { dataTransfer });
  fireEvent.dragEnter(tiles[1]);
  expect(onPreview).toHaveBeenLastCalledWith(ids[0], 1);
  fireEvent.drop(screen.getByTestId("sheet-reorder-grid"));
  expect(onDrop).toHaveBeenCalledOnce();
  fireEvent.dragEnd(tiles[0]);
  expect(onCancel).not.toHaveBeenCalled();
  expect(dataTransfer.effectAllowed).toBe("move");
});

test("keeps Grade structural gestures disabled during Sheet Edit Mode", () => {
  const onPreview = vi.fn();
  const ids = projection.state.album.sheets.map((sheet) => sheet.id);
  render(
    <InspectorPanel
      {...props()}
      sheetReorder={{
        disabled: true,
        onCancel: vi.fn(),
        onDrop: vi.fn(),
        onPreview,
        representation: {
          ghost: null,
          order: ids,
          placeholderIndex: null,
        },
        status: "idle",
      }}
    />,
  );
  const tile = document.querySelector("[data-sheet-id]")!;
  expect(tile).toHaveAttribute("draggable", "false");
  fireEvent.dragStart(tile, { dataTransfer: { setData: vi.fn() } });
  expect(onPreview).not.toHaveBeenCalled();
});

test("announces an invalid Grade target without inventing a placeholder", () => {
  const ids = projection.state.album.sheets.map((sheet) => sheet.id);
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

test("keeps progressive Grade auto-scroll running between dragover events", () => {
  const frames = new Map<number, FrameRequestCallback>();
  let nextFrameId = 1;
  const requestFrame = vi
    .spyOn(window, "requestAnimationFrame")
    .mockImplementation((callback) => {
      const frameId = nextFrameId;
      nextFrameId += 1;
      frames.set(frameId, callback);
      return frameId;
    });
  const cancelFrame = vi
    .spyOn(window, "cancelAnimationFrame")
    .mockImplementation((frameId) => {
      frames.delete(frameId);
    });
  const onDrop = vi.fn();
  const ids = projection.state.album.sheets.map((sheet) => sheet.id);
  const { unmount } = render(
    <InspectorPanel
      {...props()}
      sheetReorder={{
        disabled: false,
        onCancel: vi.fn(),
        onDrop,
        onPreview: vi.fn(),
        representation: {
          ghost: null,
          order: ids,
          placeholderIndex: null,
        },
        status: "idle",
      }}
    />,
  );
  try {
    const viewport = document.querySelector<HTMLElement>(
      ".inspector-scroll",
    )!;
    viewport.scrollTop = 100;
    vi.spyOn(viewport, "getBoundingClientRect").mockReturnValue({
      bottom: 400,
      height: 400,
      left: 0,
      right: 300,
      top: 0,
      width: 300,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    const grid = screen.getByTestId("sheet-reorder-grid");
    const firstTile = document.querySelector<HTMLElement>(
      `[data-sheet-id="${ids[0]}"]`,
    )!;
    const dataTransfer = {
      effectAllowed: "none",
      setData: vi.fn(),
    };
    fireEvent.dragStart(firstTile, { dataTransfer });

    dragOverGridAt(grid, 450);
    expect(requestFrame).toHaveBeenCalledOnce();

    runNextFrame(frames, 1_000);
    expect(viewport.scrollTop).toBe(100);
    runNextFrame(frames, 1_016);
    expect(viewport.scrollTop).toBeCloseTo(111.52);
    runNextFrame(frames, 1_032);
    expect(viewport.scrollTop).toBeCloseTo(123.04);

    dragOverGridAt(grid, 364);
    runNextFrame(frames, 1_048);
    expect(viewport.scrollTop).toBeCloseTo(125.92);

    const pendingFrameId = [...frames.keys()][0];
    fireEvent.drop(grid);
    expect(cancelFrame).toHaveBeenLastCalledWith(pendingFrameId);
    expect(frames.size).toBe(0);
    expect(onDrop).toHaveBeenCalledOnce();
  } finally {
    unmount();
    vi.restoreAllMocks();
  }
});

test.each(["dragend", "Escape", "unmount"] as const)(
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
    const ids = projection.state.album.sheets.map((sheet) => sheet.id);
    const view = render(
      <InspectorPanel
        {...props()}
        sheetReorder={{
          disabled: false,
          onCancel,
          onDrop: vi.fn(),
          onPreview: vi.fn(),
          representation: {
            ghost: null,
            order: ids,
            placeholderIndex: null,
          },
          status: "idle",
        }}
      />,
    );
    let mounted = true;
    try {
      const viewport = document.querySelector<HTMLElement>(
        ".inspector-scroll",
      )!;
      vi.spyOn(viewport, "getBoundingClientRect").mockReturnValue({
        bottom: 400,
        height: 400,
        left: 0,
        right: 300,
        top: 0,
        width: 300,
        x: 0,
        y: 0,
        toJSON: () => ({}),
      });
      const grid = screen.getByTestId("sheet-reorder-grid");
      const firstTile = document.querySelector<HTMLElement>(
        `[data-sheet-id="${ids[0]}"]`,
      )!;
      fireEvent.dragStart(firstTile, {
        dataTransfer: { effectAllowed: "none", setData: vi.fn() },
      });
      dragOverGridAt(grid, 450);
      expect(requestFrame).toHaveBeenCalledOnce();

      if (termination === "dragend") {
        fireEvent.dragEnd(firstTile);
      } else if (termination === "Escape") {
        fireEvent.keyDown(window, { key: "Escape" });
      } else {
        view.unmount();
        mounted = false;
      }

      expect(cancelFrame).toHaveBeenCalledWith(41);
      expect(frames.size).toBe(0);
      if (termination !== "unmount") {
        expect(onCancel).toHaveBeenCalledOnce();
      }
    } finally {
      if (mounted) view.unmount();
      vi.restoreAllMocks();
    }
  },
);

function dragOverGridAt(target: HTMLElement, clientY: number) {
  const event = createEvent.dragOver(target);
  Object.defineProperty(event, "clientY", { value: clientY });
  fireEvent(target, event);
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
