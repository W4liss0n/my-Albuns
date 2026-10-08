import { afterEach, expect, test } from "vitest";

import { CanvasRenderLoop, wakeOnCanvasInput, type CanvasRenderTicker } from "./canvasRenderLoop";

// Mirrors Pixi's Ticker timing: start() stamps lastTime with performance.now(),
// and a frame runs its listeners only when its timestamp is later than lastTime.
class FakeTicker implements CanvasRenderTicker {
  started: boolean;
  lastTime = -1;
  renders = 0;
  private listeners: Array<{ fn: () => void; context: unknown; priority: number }> = [];

  constructor(started = false, withRender = true) {
    this.started = started;
    // Pixi's TickerPlugin registers app.render at UPDATE_PRIORITY.LOW.
    if (withRender) this.add(() => { this.renders += 1; }, undefined, -25);
  }

  get count() { return this.listeners.length; }
  add(fn: () => void, context?: unknown, priority = 0) {
    const index = this.listeners.findIndex((listener) => priority > listener.priority);
    const listener = { fn, context, priority };
    if (index < 0) this.listeners.push(listener);
    else this.listeners.splice(index, 0, listener);
  }
  remove(fn: () => void, context?: unknown) {
    this.listeners = this.listeners.filter((listener) => listener.fn !== fn || listener.context !== context);
  }
  start() {
    if (this.started) return;
    this.started = true;
    if (this.listeners.length > 0) this.lastTime = clock;
  }
  stop() { this.started = false; }
  frame(time = clock) {
    if (!this.started) return;
    if (time > this.lastTime) {
      for (const listener of [...this.listeners]) listener.fn.call(listener.context);
    }
    this.lastTime = time;
  }
}

let clock = 0;
let timelineTime: number | null = null;
const now = () => clock;
const frameTime = () => timelineTime;
const loops: CanvasRenderLoop[] = [];

function createLoop(ticker = new FakeTicker(), sharedTickers: FakeTicker[] = []) {
  const loop = new CanvasRenderLoop({ ticker, sharedTickers, now, frameTime, idleMs: 500 });
  loops.push(loop);
  return loop;
}

afterEach(() => {
  for (const loop of loops.splice(0)) loop.destroy();
  clock = 0;
  timelineTime = null;
  delete window.__myalbunsCanvasRenderStats;
});

test("renders after a wake and stops once the idle delay has passed", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  expect(ticker.started).toBe(false);

  loop.wake();
  expect(ticker.started).toBe(true);
  ticker.frame();
  clock = 499;
  ticker.frame();
  expect(ticker.started).toBe(true);
  expect(ticker.renders).toBe(2);

  clock = 500;
  ticker.frame();
  // The settle runs after the render, so the last change is still drawn.
  expect(ticker.renders).toBe(3);
  expect(ticker.started).toBe(false);
  ticker.frame();
  expect(ticker.renders).toBe(3);
});

test("the frame an input handler runs in draws after a wake from idle", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  loop.wake();
  clock = 1_000;
  ticker.frame();
  expect(ticker.started).toBe(false);

  // The handler runs 6 ms into a frame whose rAF timestamp is 2_000.
  timelineTime = 2_000;
  clock = 2_006;
  loop.wake();
  // Only a millisecond before the frame, so the first delta stays small.
  expect(ticker.lastTime).toBe(1_999);
  ticker.frame(2_000);
  expect(ticker.renders).toBe(2);
});

test("without a timeline time the first frame after a wake still draws", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  clock = 2_000;
  loop.wake();
  expect(ticker.lastTime).toBe(1_983);
  ticker.frame(1_990);
  expect(ticker.renders).toBe(1);
});

test("a wake leaves the timing of a running ticker alone", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  loop.wake();
  clock = 100;
  ticker.frame();
  timelineTime = 50;
  loop.wake();
  expect(ticker.lastTime).toBe(100);
});

test("counts wakes from idle and rendered frames in development", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  loop.wake();
  loop.wake();
  ticker.frame();
  clock = 600;
  ticker.frame();
  loop.wake();
  expect(window.__myalbunsCanvasRenderStats).toEqual({ wakes: 2, frames: 2 });
});

test("a wake extends the awake window and never shortens it", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  loop.wake(1_000);
  clock = 100;
  loop.wake(50);
  clock = 900;
  ticker.frame();
  expect(ticker.started).toBe(true);
  loop.wake();
  clock = 1_300;
  ticker.frame();
  expect(ticker.started).toBe(true);
  clock = 1_400;
  ticker.frame();
  expect(ticker.started).toBe(false);
});

test("wakes again after it stopped", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  loop.wake();
  clock = 600;
  ticker.frame();
  expect(ticker.started).toBe(false);

  loop.wake();
  expect(ticker.started).toBe(true);
  ticker.frame();
  expect(ticker.renders).toBe(2);
});

test("keeps rendering while held, including nested holds, then settles", () => {
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  const releaseOuter = loop.hold();
  const releaseInner = loop.hold();
  expect(ticker.started).toBe(true);

  clock = 10_000;
  ticker.frame();
  releaseInner();
  releaseInner();
  clock = 20_000;
  ticker.frame();
  expect(ticker.started).toBe(true);

  releaseOuter();
  clock = 20_499;
  ticker.frame();
  expect(ticker.started).toBe(true);
  clock = 20_500;
  ticker.frame();
  expect(ticker.started).toBe(false);
});

test("stops a shared ticker only when every loop driving it is idle", () => {
  const system = new FakeTicker(true, false);
  const first = createLoop(new FakeTicker(), [system]);
  const second = createLoop(new FakeTicker(), [system]);
  first.wake(100);
  const release = second.hold();

  clock = 200;
  system.frame();
  expect(system.started).toBe(true);

  release();
  clock = 1_000;
  system.frame();
  expect(system.started).toBe(false);

  first.wake();
  expect(system.started).toBe(true);
});

test("destroy removes its listeners and leaves a shared ticker running", () => {
  const ticker = new FakeTicker();
  const system = new FakeTicker(true, false);
  const loop = createLoop(ticker, [system]);
  loop.wake();
  clock = 600;
  system.frame();
  expect(system.started).toBe(false);

  loop.destroy();
  expect(ticker.count).toBe(1);
  expect(system.count).toBe(0);
  expect(system.started).toBe(true);

  ticker.stop();
  loop.wake();
  loop.hold()();
  expect(ticker.started).toBe(false);
});

function dispatchTrusted(target: EventTarget, event: Event) {
  const impl = (object: object) => (object as Record<symbol, { isTrusted?: boolean; _dispatch?: (event: unknown) => boolean }>)[
    Object.getOwnPropertySymbols(object).find((symbol) => symbol.description === "impl")!
  ];
  const eventImpl = impl(event);
  eventImpl.isTrusted = true;
  impl(target)._dispatch!(eventImpl);
}

function inputFixture() {
  const host = document.createElement("div");
  const canvas = document.createElement("canvas");
  host.appendChild(canvas);
  document.body.appendChild(host);
  const outside = document.createElement("button");
  document.body.appendChild(outside);
  const ticker = new FakeTicker();
  const loop = createLoop(ticker);
  const detach = wakeOnCanvasInput(host, loop);
  const settle = () => { clock += 10_000; ticker.frame(); };
  return {
    canvas, outside, ticker, settle,
    dispose: () => { detach(); host.remove(); outside.remove(); },
  };
}

test("trusted input inside the host wakes the loop; outside or synthetic input does not", () => {
  const { canvas, outside, ticker, dispose } = inputFixture();

  canvas.dispatchEvent(new PointerEvent("pointermove", { bubbles: true }));
  document.dispatchEvent(new PointerEvent("pointermove"));
  dispatchTrusted(outside, new PointerEvent("pointermove", { bubbles: true }));
  dispatchTrusted(outside, new WheelEvent("wheel", { bubbles: true }));
  expect(ticker.started).toBe(false);

  for (const event of [
    new PointerEvent("pointermove", { bubbles: true }),
    new WheelEvent("wheel", { bubbles: true }),
    new KeyboardEvent("keydown", { bubbles: true }),
    new Event("dragover", { bubbles: true }),
  ]) {
    ticker.stop();
    dispatchTrusted(canvas, event);
    expect(ticker.started, event.type).toBe(true);
  }
  dispose();
});

test("holds while a button pressed on the host stays down", () => {
  const { canvas, outside, ticker, settle, dispose } = inputFixture();

  dispatchTrusted(canvas, new PointerEvent("pointerdown", { bubbles: true, buttons: 1 }));
  settle();
  expect(ticker.started).toBe(true);
  // An unrelated element losing focus must not end the press.
  dispatchTrusted(outside, new FocusEvent("blur"));
  settle();
  expect(ticker.started).toBe(true);

  dispatchTrusted(outside, new PointerEvent("pointerup", { bubbles: true, buttons: 0 }));
  ticker.frame();
  expect(ticker.started).toBe(true);
  settle();
  expect(ticker.started).toBe(false);

  dispatchTrusted(canvas, new PointerEvent("pointerdown", { bubbles: true, buttons: 1 }));
  dispatchTrusted(outside, new PointerEvent("pointermove", { bubbles: true, buttons: 0 }));
  settle();
  expect(ticker.started).toBe(false);

  dispatchTrusted(canvas, new PointerEvent("pointerdown", { bubbles: true, buttons: 1 }));
  window.dispatchEvent(new FocusEvent("blur"));
  settle();
  expect(ticker.started).toBe(false);
  dispose();
});

test("window resize, focus and becoming visible wake the loop", () => {
  const { ticker, dispose } = inputFixture();
  for (const [target, event] of [
    [window, new Event("resize")],
    [window, new FocusEvent("focus")],
    [document, new Event("visibilitychange")],
  ] as const) {
    ticker.stop();
    target.dispatchEvent(event);
    expect(ticker.started, event.type).toBe(true);
  }
  dispose();
});
