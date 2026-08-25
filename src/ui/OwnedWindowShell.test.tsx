import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import { OwnedWindowShell } from "./OwnedWindowShell";
import { WindowControlsProvider } from "./WindowControlsContext";

let contentHeight = 0;
let resizeCallback: ResizeObserverCallback | null = null;

class ResizeObserverMock {
  constructor(callback: ResizeObserverCallback) {
    resizeCallback = callback;
  }

  disconnect = vi.fn();
  observe = vi.fn();
  unobserve = vi.fn();
}

beforeEach(() => {
  contentHeight = 198;
  resizeCallback = null;
  vi.stubGlobal("ResizeObserver", ResizeObserverMock);
  vi.spyOn(HTMLElement.prototype, "scrollHeight", "get").mockImplementation(
    function measuredScrollHeight(this: HTMLElement) {
      return this.classList.contains("ui-owned-window-shell")
        ? contentHeight
        : 0;
    },
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

test("fits an owned window to its initial and changing content height", () => {
  const fitContent = vi.fn();
  const controls = {
    close: vi.fn(),
    fitContent,
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
  };

  render(
    <WindowControlsProvider controls={controls}>
      <OwnedWindowShell>
        <div>Conteúdo curto</div>
      </OwnedWindowShell>
    </WindowControlsProvider>,
  );

  expect(fitContent).toHaveBeenLastCalledWith(198);

  contentHeight = 264;
  act(() => {
    resizeCallback?.([], {} as ResizeObserver);
  });
  expect(fitContent).toHaveBeenLastCalledWith(264);
});
