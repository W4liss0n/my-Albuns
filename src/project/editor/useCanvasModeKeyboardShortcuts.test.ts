import { fireEvent, renderHook } from "@testing-library/react";
import { useState } from "react";
import { flushSync } from "react-dom";
import { afterEach, expect, test, vi } from "vitest";

import type { AlbumCanvasMode } from "../canvas/albumCanvasContract";
import { useCanvasModeKeyboardShortcuts } from "./useCanvasModeKeyboardShortcuts";

const editing: AlbumCanvasMode = { kind: "sheet-editing", sheetId: "sheet-001" };

afterEach(() => {
  document.body.replaceChildren();
});

function renderModeShortcuts(initialMode: AlbumCanvasMode, interactionBlocked = false) {
  const canvasHost = document.createElement("div");
  const canvas = document.createElement("canvas");
  canvasHost.className = "canvas-host";
  canvas.tabIndex = 0;
  canvasHost.append(canvas);
  document.body.append(canvasHost);
  const enter = vi.fn<(sheetId: string) => void>();
  const exit = vi.fn<() => void>();
  const view = renderHook(() => {
    const [mode, setMode] = useState(initialMode);
    // flushSync re-subscribes the window listener while the same keydown is
    // still dispatching, the worst case for a toggle.
    useCanvasModeKeyboardShortcuts({
      implicitSheetId: "sheet-001",
      interactionBlocked,
      mode,
      onEnterSheetEditing: (sheetId) => {
        enter(sheetId);
        flushSync(() => setMode({ kind: "sheet-editing", sheetId }));
      },
      onExitSheetEditing: () => {
        exit();
        flushSync(() => setMode({ kind: "normal" }));
      },
    });
    return mode;
  });
  return { canvas, enter, exit, view };
}

test("Enter on the Canvas toggles Sheet editing without undoing itself in the same press", () => {
  const { canvas, enter, exit, view } = renderModeShortcuts({ kind: "normal" });

  expect(fireEvent.keyDown(canvas, { key: "Enter" })).toBe(false);
  expect(view.result.current).toEqual(editing);
  expect(enter).toHaveBeenCalledExactlyOnceWith("sheet-001");
  expect(exit).not.toHaveBeenCalled();

  expect(fireEvent.keyDown(canvas, { key: "Enter" })).toBe(false);
  expect(view.result.current).toEqual({ kind: "normal" });
  expect(enter).toHaveBeenCalledOnce();
  expect(exit).toHaveBeenCalledOnce();
});

test("holding Enter does not toggle Sheet editing back and forth", () => {
  const { canvas, enter, exit, view } = renderModeShortcuts({ kind: "normal" });

  fireEvent.keyDown(canvas, { key: "Enter" });
  for (let repeat = 0; repeat < 3; repeat += 1) {
    expect(fireEvent.keyDown(canvas, { key: "Enter", repeat: true })).toBe(true);
  }
  expect(view.result.current).toEqual(editing);

  fireEvent.keyDown(canvas, { key: "Enter" });
  fireEvent.keyDown(canvas, { key: "Enter", repeat: true });
  expect(view.result.current).toEqual({ kind: "normal" });
  expect(enter).toHaveBeenCalledOnce();
  expect(exit).toHaveBeenCalledOnce();
});

test("controls, menus, modified Enter and handled Enter keep Sheet editing", () => {
  const { canvas, exit, view } = renderModeShortcuts(editing);
  const inspector = document.createElement("section");
  const field = document.createElement("input");
  const slider = document.createElement("input");
  const button = document.createElement("button");
  const menu = document.createElement("div");
  const menuItem = document.createElement("button");
  const canvasControl = document.createElement("button");
  slider.type = "range";
  menu.setAttribute("role", "menu");
  menuItem.setAttribute("role", "menuitem");
  menu.append(menuItem);
  inspector.append(field, slider, button);
  canvas.parentElement!.append(canvasControl);
  document.body.append(inspector, menu);

  for (const target of [field, slider, button, menuItem, canvasControl]) {
    expect(fireEvent.keyDown(target, { key: "Enter" })).toBe(true);
  }
  for (const modifier of ["ctrlKey", "altKey", "metaKey", "shiftKey"]) {
    fireEvent.keyDown(canvas, { key: "Enter", [modifier]: true });
  }
  canvas.addEventListener("keydown", (event) => event.preventDefault(), { once: true });
  fireEvent.keyDown(canvas, { key: "Enter" });

  expect(view.result.current).toEqual(editing);
  expect(exit).not.toHaveBeenCalled();
});

test("Enter leaves only with focus on the Canvas, not from other focusable panels", () => {
  const { exit, view } = renderModeShortcuts(editing);
  const mediaGrid = document.createElement("div");
  const splitter = document.createElement("div");
  mediaGrid.setAttribute("role", "group");
  mediaGrid.tabIndex = -1;
  splitter.setAttribute("role", "separator");
  splitter.tabIndex = 0;
  document.body.append(mediaGrid, splitter);

  for (const target of [mediaGrid, splitter]) {
    expect(fireEvent.keyDown(target, { key: "Enter" })).toBe(true);
  }

  expect(view.result.current).toEqual(editing);
  expect(exit).not.toHaveBeenCalled();
});

test("Enter also leaves from the body that a removed Sheet bar leaves focused", () => {
  const { exit, view } = renderModeShortcuts(editing);

  fireEvent.keyDown(document.body, { key: "Enter" });

  expect(view.result.current).toEqual({ kind: "normal" });
  expect(exit).toHaveBeenCalledOnce();
});

test("blocked interaction refuses entering but, as with Escape, not leaving", () => {
  const blocked = renderModeShortcuts({ kind: "normal" }, true);
  fireEvent.keyDown(blocked.canvas, { key: "Enter" });
  expect(blocked.view.result.current).toEqual({ kind: "normal" });
  blocked.view.unmount();

  const editingBlocked = renderModeShortcuts(editing, true);
  fireEvent.keyDown(editingBlocked.canvas, { key: "Enter" });
  expect(editingBlocked.view.result.current).toEqual({ kind: "normal" });
});

test("Escape still leaves Sheet editing from any target", () => {
  const { exit, view } = renderModeShortcuts(editing);
  const field = document.createElement("input");
  document.body.append(field);

  expect(fireEvent.keyDown(field, { key: "Escape" })).toBe(false);

  expect(view.result.current).toEqual({ kind: "normal" });
  expect(exit).toHaveBeenCalledOnce();
});
