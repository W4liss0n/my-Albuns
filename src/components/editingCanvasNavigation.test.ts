import { fireEvent } from "@testing-library/dom";
import { expect, test, vi } from "vitest";
import type { AlbumCanvasProps } from "./albumCanvasContract";
import { EditingCanvasNavigation } from "./editingCanvasNavigation";

test("Space with a pointer moves the editing camera and cancels the viewer tap", () => {
  const canvas = document.createElement("canvas");
  canvas.tabIndex = 0;
  document.body.appendChild(canvas);
  canvas.getBoundingClientRect = () => ({ x: 0, y: 0, left: 0, top: 0, right: 600, bottom: 400, width: 600, height: 400, toJSON: () => undefined });
  canvas.setPointerCapture = vi.fn();
  canvas.releasePointerCapture = vi.fn();
  const cancelViewerTap = vi.fn();
  const input = { projectId: "project", mode: { kind: "sheet-editing", sheetId: "sheet" },
    editingNavigation: { disabled: false, fitRequest: 0, onPanGesture: cancelViewerTap } } as unknown as AlbumCanvasProps;
  const fit = { width: 600, height: 400, sheetWidth: 400, sheetHeight: 250, scale: 1 };
  const navigation = new EditingCanvasNavigation(canvas, () => input, vi.fn(), vi.fn());
  try {
    navigation.synchronize(input, fit);
    fireEvent.wheel(canvas, { ctrlKey: true, deltaY: -500, clientX: 300, clientY: 200 });
    const before = navigation.synchronize(input, fit);
    expect(before.zoom).toBeGreaterThan(1);
    canvas.focus();
    fireEvent.keyDown(canvas, { key: " ", code: "Space" });
    fireEvent.pointerDown(canvas, { button: 0, pointerId: 1, clientX: 300, clientY: 200 });
    fireEvent.pointerMove(canvas, { pointerId: 1, clientX: 340, clientY: 220 });
    fireEvent.pointerUp(canvas, { pointerId: 1, clientX: 340, clientY: 220 });
    fireEvent.keyUp(canvas, { key: " ", code: "Space" });
    const after = navigation.synchronize(input, fit);
    expect(cancelViewerTap).toHaveBeenCalledOnce();
    expect(after.x).not.toBe(before.x);
  } finally {
    navigation.destroy();
    canvas.remove();
  }
});
