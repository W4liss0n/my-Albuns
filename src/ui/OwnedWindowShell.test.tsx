import { act, fireEvent, render } from "@testing-library/react";
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
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
    function measuredBorderBox(this: HTMLElement) {
      return new DOMRect(0, 0, 520,
        this.classList.contains("ui-owned-window-shell") ? contentHeight + 2 : 0);
    },
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

test("fits an owned window to its initial and changing content height", () => {
  const measuredHeights: number[] = [];
  const fitContent = vi.fn((measureHeight: () => number) => {
    measuredHeights.push(measureHeight());
  });
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

  expect(measuredHeights).toEqual([200]);

  contentHeight = 264;
  act(() => {
    resizeCallback?.([], {} as ResizeObserver);
  });
  expect(measuredHeights).toEqual([200, 266]);
});

test("Escape does what the close button does, unless the content took it", () => {
  const close = vi.fn();
  const controls = { close, fitContent: vi.fn(), minimize: vi.fn(), toggleMaximize: vi.fn() };
  const view = render(
    <WindowControlsProvider controls={controls}>
      <OwnedWindowShell controls="close">
        <input aria-label="Campo" onKeyDown={(event) => event.preventDefault()} />
      </OwnedWindowShell>
    </WindowControlsProvider>,
  );

  fireEvent.keyDown(view.getByLabelText("Campo"), { key: "Escape" });
  expect(close).not.toHaveBeenCalled();
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(close).toHaveBeenCalledOnce();

  view.rerender(
    <WindowControlsProvider controls={controls}>
      <OwnedWindowShell controls="none"><p>Sem fechar</p></OwnedWindowShell>
    </WindowControlsProvider>,
  );
  fireEvent.keyDown(document.body, { key: "Escape" });
  expect(close).toHaveBeenCalledOnce();
});
