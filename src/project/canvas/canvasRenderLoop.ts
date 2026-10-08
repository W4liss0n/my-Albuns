/** The part of a Pixi Ticker the render loop drives. */
export interface CanvasRenderTicker {
  readonly started: boolean;
  /** Pixi runs a frame's listeners only when its timestamp is later than this. */
  lastTime: number;
  start(): void;
  stop(): void;
  add(fn: () => void, context?: unknown, priority?: number): unknown;
  remove(fn: () => void, context?: unknown): unknown;
}

/** What scene code needs: ask for frames now, or keep them coming. */
export interface CanvasRenderDemand {
  wake(ms?: number): void;
  hold(): () => void;
}

export const idleCanvasRenderDemand: CanvasRenderDemand = {
  wake: () => undefined,
  hold: () => () => undefined,
};

export const CANVAS_RENDER_IDLE_MS = 500;
// One display frame at 60 Hz, plus a millisecond so its timestamp passes Pixi's guard.
const FRAME_FALLBACK_MS = 17;
// Pixi registers app.render at UPDATE_PRIORITY.LOW (-25); UTILITY (-50) runs after it.
const SETTLE_PRIORITY = -50;
// A shared ticker (Ticker.system) stops only when every loop that drives it is idle.
const sharedTickerOwners = new Map<CanvasRenderTicker, Set<CanvasRenderLoop>>();

interface CanvasRenderLoopOptions {
  ticker: CanvasRenderTicker;
  sharedTickers?: readonly CanvasRenderTicker[];
  now?: () => number;
  /** The timestamp of the display frame being processed, as rAF will report it. */
  frameTime?: () => number | null;
  idleMs?: number;
}

/** Development-only counters, read from the DevTools console of a Project window. */
export interface CanvasRenderStats {
  /** Times a loop started its stopped ticker. */
  wakes: number;
  /** Frames rendered by every loop of the window. */
  frames: number;
}

declare global {
  interface Window {
    __myalbunsCanvasRenderStats?: CanvasRenderStats;
  }
}

function countRenderStat(key: keyof CanvasRenderStats) {
  // A constant false in production builds, so the bundler drops the counter.
  if (import.meta.env.DEV && typeof window !== "undefined") {
    const stats = (window.__myalbunsCanvasRenderStats ??= { wakes: 0, frames: 0 });
    stats[key] += 1;
  }
}

function documentFrameTime(): number | null {
  const time = typeof document === "undefined" ? null : document.timeline?.currentTime;
  return typeof time === "number" ? time : null;
}

/**
 * Runs the Pixi tickers only while something can change on screen. An idle window
 * that redrew every display refresh cost ~30% of a GPU core and slowed the
 * foreground editor's frames 4x with six Projects open (measured 2026-10-07).
 */
export class CanvasRenderLoop implements CanvasRenderDemand {
  private readonly ticker: CanvasRenderTicker;
  private readonly sharedTickers: readonly CanvasRenderTicker[];
  private readonly now: () => number;
  private readonly frameTime: () => number | null;
  private readonly idleMs: number;
  private readonly settles = new Map<CanvasRenderTicker, () => void>();
  private awakeUntil = -Infinity;
  private holds = 0;
  private destroyed = false;

  constructor({
    ticker,
    sharedTickers = [],
    now = () => performance.now(),
    frameTime = documentFrameTime,
    idleMs = CANVAS_RENDER_IDLE_MS,
  }: CanvasRenderLoopOptions) {
    this.ticker = ticker;
    this.sharedTickers = sharedTickers;
    this.now = now;
    this.frameTime = frameTime;
    this.idleMs = idleMs;
    this.attachSettle(ticker, () => {
      countRenderStat("frames");
      if (!this.awake) ticker.stop();
    });
    for (const shared of sharedTickers) {
      const owners = sharedTickerOwners.get(shared) ?? new Set<CanvasRenderLoop>();
      sharedTickerOwners.set(shared, owners);
      owners.add(this);
      this.attachSettle(shared, () => {
        if ([...owners].every((owner) => !owner.awake)) shared.stop();
      });
    }
  }

  get awake() {
    return !this.destroyed && (this.holds > 0 || this.now() < this.awakeUntil);
  }

  wake(ms = this.idleMs) {
    if (this.destroyed) return;
    this.awakeUntil = Math.max(this.awakeUntil, this.now() + ms);
    this.start();
  }

  hold() {
    if (this.destroyed) return () => undefined;
    this.holds += 1;
    this.start();
    let released = false;
    return () => {
      if (released) return;
      released = true;
      this.holds -= 1;
      // Render the frames that follow the release, then settle.
      this.wake();
    };
  }

  destroy() {
    if (this.destroyed) return;
    this.destroyed = true;
    for (const [ticker, settle] of this.settles) ticker.remove(settle, this);
    this.settles.clear();
    for (const shared of this.sharedTickers) {
      const owners = sharedTickerOwners.get(shared);
      owners?.delete(this);
      if (owners && owners.size > 0) continue;
      sharedTickerOwners.delete(shared);
      // Leave the shared ticker as Pixi expects it: running.
      if (!shared.started) shared.start();
    }
  }

  private attachSettle(ticker: CanvasRenderTicker, settle: () => void) {
    this.settles.set(ticker, settle);
    ticker.add(settle, this, SETTLE_PRIORITY);
  }

  private start() {
    if (!this.ticker.started) {
      this.startTicker(this.ticker);
      countRenderStat("wakes");
    }
    for (const shared of this.sharedTickers) {
      if (!shared.started) this.startTicker(shared);
    }
  }

  /**
   * Pixi's start() stamps lastTime with performance.now(), but the next rAF
   * timestamp is the start of the frame an input handler runs in, which is
   * earlier: that frame would draw nothing. Stamp it just before the frame
   * instead; not -1, or the first delta would jump by Pixi's 100 ms cap.
   */
  private startTicker(ticker: CanvasRenderTicker) {
    ticker.start();
    const frameTime = this.frameTime();
    const beforeFrame = frameTime === null ? this.now() - FRAME_FALLBACK_MS : frameTime - 1;
    ticker.lastTime = Math.min(ticker.lastTime, beforeFrame);
  }
}

/**
 * Wakes the loop for trusted input inside the Canvas host and holds it while a
 * button is pressed there (a drag with a resting pointer still auto-scrolls).
 * Registered on window in capture phase so no later handler can swallow it.
 * Pixi's synthetic hover moves are untrusted and target the document: ignored.
 */
export function wakeOnCanvasInput(host: HTMLElement, loop: CanvasRenderDemand) {
  let releasePress: (() => void) | null = null;
  const release = () => {
    releasePress?.();
    releasePress = null;
  };
  const inside = (event: Event) =>
    event.isTrusted && event.target instanceof Node && host.contains(event.target);
  const wakeInside = (event: Event) => { if (inside(event)) loop.wake(); };
  const pointerDown = (event: Event) => {
    if (!inside(event)) return;
    loop.wake();
    releasePress ??= loop.hold();
  };
  const pointerMove = (event: Event) => {
    if (!event.isTrusted) return;
    // A release outside the window may never reach it.
    if (releasePress && (event as PointerEvent).buttons === 0) release();
    wakeInside(event);
  };
  const pointerEnd = (event: Event) => {
    if (!event.isTrusted) return;
    if (event.type === "pointercancel" || (event as PointerEvent).buttons === 0) release();
    wakeInside(event);
  };
  const visibilityChange = () => {
    if (document.hidden) release();
    else loop.wake();
  };
  // Focus events of page elements also reach a window capture listener.
  const blur = (event: Event) => { if (!(event.target instanceof Node)) release(); };
  const focus = (event: Event) => { if (!(event.target instanceof Node)) loop.wake(); };
  const wakeWindow = () => loop.wake();
  const options = { capture: true, passive: true } as const;
  const windowListeners: Array<[string, (event: Event) => void]> = [
    ["pointerdown", pointerDown],
    ["pointermove", pointerMove],
    ["pointerup", pointerEnd],
    ["pointercancel", pointerEnd],
    ...["pointerover", "pointerout", "pointerenter", "pointerleave", "wheel", "contextmenu",
      "dragenter", "dragover", "dragleave", "drop", "keydown", "keyup"]
      .map((type) => [type, wakeInside] as [string, (event: Event) => void]),
    ["blur", blur],
    ["focus", focus],
    ["resize", wakeWindow],
  ];
  for (const [type, listener] of windowListeners) window.addEventListener(type, listener, options);
  document.addEventListener("visibilitychange", visibilityChange);
  return () => {
    release();
    for (const [type, listener] of windowListeners) window.removeEventListener(type, listener, options);
    document.removeEventListener("visibilitychange", visibilityChange);
  };
}
