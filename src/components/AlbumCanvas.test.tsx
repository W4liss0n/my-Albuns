import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  CompositionPlan,
  PhotoPlacementPlan,
} from "../domain/project";
import {
  AlbumCanvas,
  type PhotoTransformPreview,
} from "./AlbumCanvas";

const pixiLifecycle = vi.hoisted(() => ({
  displays: [] as Array<{
    alpha: number;
    children: unknown[];
    handlers: Map<string, (event: unknown) => void>;
    label: string;
    mask: unknown;
    pathCommands: Array<{
      kind: "lineTo" | "moveTo";
      x: number;
      y: number;
    }>;
    position: { x: number; y: number };
    scale: { x: number; y: number };
    visible: boolean;
    emit(name: string, event: unknown): void;
  }>,
  initOptions: [] as Array<Record<string, unknown>>,
  instances: [] as Array<{
    canvas: HTMLCanvasElement;
    destroyCount: number;
    initialized: boolean;
    resizeCount: number;
    screen: { width: number; height: number };
    stage: {
      children: unknown[];
      emit(name: string, event: unknown): void;
    };
  }>,
  resizeCallbacks: [] as ResizeObserverCallback[],
  resolveInitializations: [] as Array<() => void>,
}));

vi.mock("pixi.js", () => {
  class Point {
    x: number;
    y: number;

    constructor(x = 0, y = x) {
      this.x = x;
      this.y = y;
    }

    set(x: number, y = x) {
      this.x = x;
      this.y = y;
    }
  }

  class DisplayObject {
    alpha = 1;
    children: DisplayObject[] = [];
    cursor = "";
    eventMode = "";
    handlers = new Map<string, (event: unknown) => void>();
    hitArea: unknown = null;
    label = "";
    mask: DisplayObject | null = null;
    parent: DisplayObject | null = null;
    pathCommands: Array<{
      kind: "lineTo" | "moveTo";
      x: number;
      y: number;
    }> = [];
    pivot = new Point();
    position = new Point();
    rotation = 0;
    scale = new Point(1, 1);
    visible = true;

    constructor() {
      pixiLifecycle.displays.push(this);
    }

    get x() {
      return this.position.x;
    }

    set x(value: number) {
      this.position.x = value;
    }

    get y() {
      return this.position.y;
    }

    set y(value: number) {
      this.position.y = value;
    }

    addChild(...children: DisplayObject[]) {
      children.forEach((child) => {
        child.parent = this;
        this.children.push(child);
      });
      return children[0];
    }

    destroy() {
      this.children = [];
      this.handlers.clear();
    }

    emit(name: string, event: unknown) {
      this.handlers.get(name)?.(event);
    }

    on(name: string, handler: (event: unknown) => void) {
      this.handlers.set(name, handler);
      return this;
    }

    removeAllListeners() {
      this.handlers.clear();
    }

    removeChildren() {
      const removed = this.children;
      this.children = [];
      return removed;
    }
  }

  class Container extends DisplayObject {}

  class Graphics extends DisplayObject {
    circle() {
      return this;
    }

    fill() {
      return this;
    }

    lineTo(x: number, y: number) {
      this.pathCommands.push({ kind: "lineTo", x, y });
      return this;
    }

    moveTo(x: number, y: number) {
      this.pathCommands.push({ kind: "moveTo", x, y });
      return this;
    }

    rect() {
      return this;
    }

    roundRect() {
      return this;
    }

    stroke() {
      return this;
    }
  }

  class Text extends DisplayObject {}

  class Application {
    canvas = document.createElement("canvas");
    destroyCount = 0;
    initialized = false;
    resizeCount = 0;
    resizeTarget: HTMLElement | null = null;
    screen = { width: 1_200, height: 500 };
    stage = new Container();

    constructor() {
      pixiLifecycle.instances.push(this);
    }

    init(options: Record<string, unknown>) {
      pixiLifecycle.initOptions.push(options);
      this.resizeTarget = options.resizeTo as HTMLElement;
      return new Promise<void>((resolve) => {
        pixiLifecycle.resolveInitializations.push(() => {
          this.initialized = true;
          resolve();
        });
      });
    }

    resize() {
      this.resizeCount += 1;
      if (!this.resizeTarget) return;
      this.screen.width =
        this.resizeTarget.clientWidth || this.screen.width;
      this.screen.height =
        this.resizeTarget.clientHeight || this.screen.height;
    }

    destroy() {
      if (!this.initialized) {
        throw new TypeError("PixiJS was destroyed before initialization");
      }
      this.destroyCount += 1;
    }
  }

  return {
    Application,
    Container,
    FederatedPointerEvent: class {},
    FederatedWheelEvent: class {},
    Graphics,
    Rectangle: class {},
    Text,
  };
});

const composition: CompositionPlan = {
  sheets: [
    {
      sheetId: "sheet-001",
      number: 1,
      widthUm: 600_000,
      heightUm: 300_000,
      hasOverlay: false,
      frames: [],
    },
  ],
};

const threeSheetComposition: CompositionPlan = {
  sheets: [1, 2, 3].map((number) => ({
    sheetId: `sheet-00${number}`,
    number,
    widthUm: 600_000,
    heightUm: 300_000,
    hasOverlay: false,
    frames: [],
  })),
};

const horizontalPlacementPlan: PhotoPlacementPlan = {
  currentPan: { x: 0, y: 0 },
  currentZoom: 1,
  panRange: { minimum: -1, maximum: 1 },
  zoomRange: { minimum: 1, maximum: 4 },
  current: {
    center: { x: 150_000, y: 100_000 },
    size: { width: 400_000, height: 200_000 },
  },
  panOrigin: { x: 150_000, y: 100_000 },
  panToCenter: {
    xx: 50_000,
    xy: 0,
    yx: 0,
    yy: 0,
  },
  centerToPan: { xx: 0.00002, xy: 0, yx: 0, yy: 0 },
  panToCenterPerZoom: {
    xx: 200_000,
    xy: 0,
    yx: 0,
    yy: 100_000,
  },
  sizePerZoom: { width: 400_000, height: 200_000 },
};

const interactiveComposition: CompositionPlan = {
  sheets: [
    {
      sheetId: "sheet-001",
      number: 1,
      widthUm: 600_000,
      heightUm: 300_000,
      hasOverlay: false,
      frames: [
        {
          frameId: "frame-001",
          clipRect: {
            x: 0,
            y: 0,
            width: 300_000,
            height: 200_000,
          },
          zIndex: 0,
          photo: {
            mediaId: "media-001",
            name: "Serra.jpg",
            drawRect: {
              x: -50_000,
              y: 0,
              width: 400_000,
              height: 200_000,
            },
            placement: horizontalPlacementPlan,
            rotationDegrees: 0,
            mirrorX: false,
            palette: ["#10202b", "#648493", "#dfa75e"],
          },
        },
      ],
    },
  ],
};

const pannedInteractiveComposition: CompositionPlan = {
  sheets: [
    {
      ...interactiveComposition.sheets[0],
      frames: [
        {
          ...interactiveComposition.sheets[0].frames[0],
          photo: {
            ...interactiveComposition.sheets[0].frames[0].photo!,
            drawRect: {
              ...interactiveComposition.sheets[0].frames[0].photo!.drawRect,
              x: -95_000,
            },
            placement: {
              ...horizontalPlacementPlan,
              currentPan: { x: -0.9, y: 0 },
              current: {
                ...horizontalPlacementPlan.current,
                center: { x: 105_000, y: 100_000 },
              },
            },
          },
        },
      ],
    },
  ],
};

const rotatedInteractiveComposition: CompositionPlan = {
  sheets: [
    {
      ...interactiveComposition.sheets[0],
      frames: [
        {
          ...interactiveComposition.sheets[0].frames[0],
          photo: {
            ...interactiveComposition.sheets[0].frames[0].photo!,
            drawRect: {
              x: -187_500,
              y: -125_000,
              width: 675_000,
              height: 450_000,
            },
            placement: {
              ...(placementFixture.cases[1]
                .expectedPlan as PhotoPlacementPlan),
              currentPan: { x: 0, y: 0 },
              current: {
                center: { x: 150_000, y: 100_000 },
                size: {
                  width: 675_000,
                  height: 450_000,
                },
              },
            },
            rotationDegrees: 90,
          },
        },
      ],
    },
  ],
};

function renderCanvas({
  compositionPlan = composition,
  onFocusSheet = vi.fn<(sheetId: string) => void>(),
  onCenteredSheetChange = vi.fn<(sheetId: string) => void>(),
  onTransformPreview = vi.fn<
    (preview: PhotoTransformPreview | null) => void
  >(),
  onPanCommit = vi.fn<
    (frameId: string, deltaX: number, deltaY: number) => void
  >(),
  onViewportChange = vi.fn<(viewport: { offsetX: number; zoom: number }) => void>(),
  onZoomCommit = vi.fn<(frameId: string, delta: number) => void>(),
  onTransformCommit = vi.fn<
    (
      frameId: string,
      deltaPanX: number,
      deltaPanY: number,
      deltaZoom: number,
    ) => void
  >(),
}: {
  compositionPlan?: CompositionPlan;
  onFocusSheet?: (sheetId: string) => void;
  onCenteredSheetChange?: (sheetId: string) => void;
  onTransformPreview?: (
    preview: PhotoTransformPreview | null,
  ) => void;
  onPanCommit?: (frameId: string, deltaX: number, deltaY: number) => void;
  onViewportChange?: (viewport: { offsetX: number; zoom: number }) => void;
  onZoomCommit?: (frameId: string, delta: number) => void;
  onTransformCommit?: (
    frameId: string,
    deltaPanX: number,
    deltaPanY: number,
    deltaZoom: number,
  ) => void;
} = {}) {
  const view = render(
    <AlbumCanvas
      composition={compositionPlan}
      selectedFrameId={null}
      focusedSheetId="sheet-001"
      centeredSheetId="sheet-001"
      viewport={{ offsetX: 42, zoom: 0.78 }}
      onSelectFrame={() => undefined}
      onFocusSheet={onFocusSheet}
      onCenteredSheetChange={onCenteredSheetChange}
      onViewportChange={onViewportChange}
      onTransformPreview={onTransformPreview}
      onPanCommit={onPanCommit}
      onZoomCommit={onZoomCommit}
      onTransformCommit={onTransformCommit}
      onMaterializedChange={() => undefined}
    />,
  );

  return {
    ...view,
    onFocusSheet,
    onCenteredSheetChange,
    onTransformPreview,
    onPanCommit,
    onViewportChange,
    onZoomCommit,
    onTransformCommit,
  };
}

async function finishPixiInitialization() {
  await act(async () => {
    pixiLifecycle.resolveInitializations[
      pixiLifecycle.resolveInitializations.length - 1
    ]?.();
    await Promise.resolve();
    await Promise.resolve();
  });
}

function displayWithHandler(name: string) {
  const display = pixiLifecycle.displays.find((candidate) =>
    candidate.handlers.has(name),
  );
  if (!display) throw new Error(`Pixi display with ${name} handler not found`);
  return display;
}

function displayWithLabel(label: string) {
  const display = pixiLifecycle.displays.find(
    (candidate) => candidate.label === label,
  );
  if (!display) throw new Error(`Pixi display labeled ${label} not found`);
  return display;
}

beforeEach(() => {
  pixiLifecycle.displays.length = 0;
  pixiLifecycle.initOptions.length = 0;
  pixiLifecycle.instances.length = 0;
  pixiLifecycle.resizeCallbacks.length = 0;
  pixiLifecycle.resolveInitializations.length = 0;
  vi.stubGlobal(
    "ResizeObserver",
    class {
      constructor(callback: ResizeObserverCallback) {
        pixiLifecycle.resizeCallbacks.push(callback);
      }

      disconnect() {}

      observe() {}

      unobserve() {}
    },
  );
});

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

test("waits for PixiJS initialization before destroying an abandoned Canvas", async () => {
  const view = renderCanvas();

  expect(pixiLifecycle.instances).toHaveLength(1);
  expect(() => view.unmount()).not.toThrow();

  await finishPixiInitialization();

  expect(pixiLifecycle.instances[0].destroyCount).toBe(1);
});

test("fits the complete sheet to the continuous Canvas at device resolution", async () => {
  renderCanvas();
  await finishPixiInitialization();

  const world = pixiLifecycle.instances[0].stage.children[0] as {
    position: { y: number };
    scale: { x: number; y: number };
  };
  const expectedScale = (500 - 2 * 24) / (300 + 24);

  expect(world.scale.x).toBeCloseTo(expectedScale, 4);
  expect(world.scale.y).toBeCloseTo(expectedScale, 4);
  expect(world.position.y - 24 * world.scale.y).toBeCloseTo(24, 4);
  expect(pixiLifecycle.initOptions[0]).toMatchObject({
    autoDensity: true,
    resolution: window.devicePixelRatio,
  });
});

test("does not zoom the continuous Canvas outside sheet-editing mode", async () => {
  const onViewportChange = vi.fn();
  renderCanvas({ onViewportChange });
  await finishPixiInitialization();
  onViewportChange.mockClear();

  pixiLifecycle.instances[0].canvas.dispatchEvent(
    new WheelEvent("wheel", {
      cancelable: true,
      ctrlKey: true,
      deltaY: -120,
    }),
  );

  expect(onViewportChange).not.toHaveBeenCalled();
});

test("keeps edge sheets centered and tracks the centered sheet while scrolling", async () => {
  const onCenteredSheetChange = vi.fn();
  const onViewportChange = vi.fn();
  renderCanvas({
    compositionPlan: threeSheetComposition,
    onCenteredSheetChange,
    onViewportChange,
  });
  await finishPixiInitialization();

  const app = pixiLifecycle.instances[0];
  const scale = (500 - 2 * 24) / (300 + 24);

  app.canvas.dispatchEvent(
    new WheelEvent("wheel", {
      cancelable: true,
      deltaY: 10_000,
    }),
  );

  const lastOffset =
    onViewportChange.mock.calls[
      onViewportChange.mock.calls.length - 1
    ]?.[0].offsetX;
  expect(lastOffset + 1_604 * scale).toBeCloseTo(600, 4);
  expect(onCenteredSheetChange).toHaveBeenLastCalledWith("sheet-003");
  onCenteredSheetChange.mockClear();

  app.canvas.dispatchEvent(
    new WheelEvent("wheel", {
      cancelable: true,
      deltaY: -10_000,
    }),
  );

  const firstOffset =
    onViewportChange.mock.calls[
      onViewportChange.mock.calls.length - 1
    ]?.[0].offsetX;
  expect(firstOffset + 300 * scale).toBeCloseTo(600, 4);
  expect(onCenteredSheetChange).not.toHaveBeenCalled();
});

test("resizes the Pixi renderer before fitting a taller Canvas", async () => {
  renderCanvas();
  await finishPixiInitialization();

  const host = document.querySelector(".canvas-host") as HTMLElement;
  Object.defineProperty(host, "clientHeight", {
    configurable: true,
    value: 700,
  });
  await act(async () => {
    pixiLifecycle.resizeCallbacks[0]?.(
      [],
      {} as ResizeObserver,
    );
    await Promise.resolve();
  });

  const world = pixiLifecycle.instances[0].stage.children[0] as {
    scale: { x: number };
  };

  expect(pixiLifecycle.instances[0].resizeCount).toBe(1);
  expect(pixiLifecycle.instances[0].screen.height).toBe(700);
  expect(world.scale.x).toBeCloseTo((700 - 2 * 24) / (300 + 24), 4);
});

test("reveals the dimmed Photo overflow and thirds guides only during Pan", async () => {
  const onPanCommit = vi.fn();
  const onTransformPreview = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onPanCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("pointerdown");
  const outsidePreview = displayWithLabel(
    "photo-pan-outside-preview",
  );
  const insidePreview = displayWithLabel(
    "photo-pan-inside-preview",
  );
  const thirdsGuides = displayWithLabel("photo-pan-thirds-guides");
  const originalPosition = { ...insidePreview.position };

  expect(outsidePreview.visible).toBe(false);
  expect(outsidePreview.alpha).toBeGreaterThan(0);
  expect(outsidePreview.alpha).toBeLessThan(1);
  expect(insidePreview.alpha).toBe(1);
  expect(thirdsGuides.visible).toBe(false);
  expect(thirdsGuides.pathCommands).toEqual([
    { kind: "moveTo", x: 100, y: 0 },
    { kind: "lineTo", x: 100, y: 200 },
    { kind: "moveTo", x: 200, y: 0 },
    { kind: "lineTo", x: 200, y: 200 },
    { kind: "moveTo", x: 0, y: 200 / 3 },
    { kind: "lineTo", x: 300, y: 200 / 3 },
    { kind: "moveTo", x: 0, y: 400 / 3 },
    { kind: "lineTo", x: 300, y: 400 / 3 },
  ]);

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });

  expect(outsidePreview.visible).toBe(true);
  expect(thirdsGuides.visible).toBe(true);

  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  expect(outsidePreview.position).toEqual(insidePreview.position);

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 0, y: 0 },
  });

  expect(outsidePreview.visible).toBe(false);
  expect(thirdsGuides.visible).toBe(false);
  expect(onPanCommit).not.toHaveBeenCalled();
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  pixiLifecycle.instances[0].stage.emit("pointercancel", {});

  expect(outsidePreview.visible).toBe(false);
  expect(thirdsGuides.visible).toBe(false);
  expect(insidePreview.position).toEqual(originalPosition);
  expect(outsidePreview.position).toEqual(originalPosition);
  expect(onPanCommit).not.toHaveBeenCalled();
  expect(onTransformPreview).toHaveBeenLastCalledWith(null);
});

test("keeps the photo inside a stationary frame mask throughout pan", async () => {
  const onPanCommit = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onPanCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("pointerdown");
  const maskedViewport = frame.children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as
    | {
        children: unknown[];
        position: { x: number; y: number };
      }
    | undefined;

  expect(maskedViewport).toBeDefined();
  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 1_000, y: 0 },
  });

  const photoLayer = maskedViewport?.children.find(
    (child) =>
      Array.isArray((child as { children?: unknown[] }).children) &&
      (child as { children: unknown[] }).children.length > 0,
  ) as { position: { x: number; y: number } } | undefined;

  expect(maskedViewport?.position).toEqual({ x: 0, y: 0 });
  expect(photoLayer).toBeDefined();
  expect(photoLayer?.position.x).toBeLessThanOrEqual(200);

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 1_000, y: 0 },
  });

  expect(onPanCommit).toHaveBeenCalledOnce();
  expect(onPanCommit).toHaveBeenCalledWith("frame-001", 1, 0);
});

test("reports the live Photo transform while Pan is moving", async () => {
  const onTransformPreview = vi.fn();
  const onPanCommit = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformPreview,
    onPanCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("pointerdown");
  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });

  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: expect.any(Number),
    panY: 0,
    zoom: 1,
  });
  expect(
    onTransformPreview.mock.calls[
      onTransformPreview.mock.calls.length - 1
    ]?.[0]?.panX,
  ).toBeGreaterThan(0);
  expect(onPanCommit).not.toHaveBeenCalled();
});

test("keeps every frame corner covered while panning a rotated photo", async () => {
  const onPanCommit = vi.fn();
  renderCanvas({
    compositionPlan: rotatedInteractiveComposition,
    onPanCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("pointerdown");
  const maskedViewport = frame.children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as { children: unknown[] };
  const photoLayer = maskedViewport.children.find(
    (child) =>
      Array.isArray((child as { children?: unknown[] }).children) &&
      (child as { children: unknown[] }).children.length > 0,
  ) as {
    position: { x: number; y: number };
    pivot: { x: number; y: number };
    rotation: number;
    scale: { x: number; y: number };
  };

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 10_000, y: 10_000 },
  });

  const cosine = Math.cos(photoLayer.rotation);
  const sine = Math.sin(photoLayer.rotation);
  for (const [cornerX, cornerY] of [
    [0, 0],
    [300, 0],
    [0, 200],
    [300, 200],
  ]) {
    const deltaX = cornerX - photoLayer.position.x;
    const deltaY = cornerY - photoLayer.position.y;
    const localX =
      (cosine * deltaX + sine * deltaY) /
      Math.abs(photoLayer.scale.x);
    const localY =
      (-sine * deltaX + cosine * deltaY) /
      Math.abs(photoLayer.scale.y);

    expect(Math.abs(localX)).toBeLessThanOrEqual(
      photoLayer.pivot.x + 0.001,
    );
    expect(Math.abs(localY)).toBeLessThanOrEqual(
      photoLayer.pivot.y + 0.001,
    );
  }

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 10_000, y: 10_000 },
  });

  expect(onPanCommit).toHaveBeenCalledOnce();
  expect(onPanCommit).toHaveBeenCalledWith("frame-001", 1, -1);
});

test("does not reset an active Pan preview when wheel Zoom starts", async () => {
  vi.useFakeTimers();
  const onTransformCommit = vi.fn();
  const canvas = renderCanvas({
    compositionPlan: interactiveComposition,
    onTransformCommit,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("pointerdown");
  const maskedViewport = frame.children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as { children: unknown[] };
  const photoLayer = maskedViewport.children.find(
    (child) =>
      Array.isArray((child as { children?: unknown[] }).children) &&
      (child as { children: unknown[] }).children.length > 0,
  ) as {
    position: { x: number; y: number };
    scale: { x: number; y: number };
  };

  frame.emit("pointerdown", {
    altKey: true,
    global: { x: 0, y: 0 },
    stopPropagation: vi.fn(),
  });
  pixiLifecycle.instances[0].stage.emit("globalpointermove", {
    global: { x: 40, y: 0 },
  });
  const pannedX = photoLayer.position.x;
  expect(pannedX).toBeGreaterThan(150);

  frame.emit("wheel", {
    altKey: true,
    deltaY: -100,
    preventDefault: vi.fn(),
  });

  expect(photoLayer.scale.y).toBeGreaterThan(1);
  expect(photoLayer.position.x).toBeCloseTo(pannedX, 4);

  await act(async () => {
    await vi.advanceTimersByTimeAsync(600);
  });
  expect(canvas.onZoomCommit).not.toHaveBeenCalled();

  pixiLifecycle.instances[0].stage.emit("pointerup", {
    global: { x: 40, y: 0 },
  });

  expect(onTransformCommit).toHaveBeenCalledOnce();
  expect(onTransformCommit.mock.calls[0][0]).toBe("frame-001");
  expect(onTransformCommit.mock.calls[0][1]).toBeGreaterThan(0);
  expect(onTransformCommit.mock.calls[0][2]).toBeCloseTo(0, 6);
  expect(onTransformCommit.mock.calls[0][3]).toBeCloseTo(0.12, 6);
  expect(canvas.onPanCommit).not.toHaveBeenCalled();
  expect(canvas.onZoomCommit).not.toHaveBeenCalled();
});

test("previews a smooth wheel zoom and commits the sequence once", async () => {
  vi.useFakeTimers();
  const onZoomCommit = vi.fn();
  const onTransformPreview = vi.fn();
  renderCanvas({
    compositionPlan: pannedInteractiveComposition,
    onZoomCommit,
    onTransformPreview,
  });
  await finishPixiInitialization();

  const frame = displayWithHandler("wheel");
  const maskedDisplay = frame.children.find(
    (child) => (child as { mask?: unknown }).mask,
  ) as {
    children: unknown[];
    position: { x: number };
    scale: { y: number };
  };
  const photoLayer =
    (maskedDisplay.children.find(
      (child) =>
        Array.isArray((child as { children?: unknown[] }).children) &&
        (child as { children: unknown[] }).children.length > 0,
    ) as
      | { position: { x: number }; scale: { y: number } }
      | undefined) ?? maskedDisplay;

  const wheel = () =>
    frame.emit("wheel", {
      altKey: true,
      deltaY: -100,
      preventDefault: vi.fn(),
    });

  wheel();
  expect(photoLayer.scale.y).toBeGreaterThan(1);
  expect(photoLayer.position.x).toBeCloseTo(83.4, 1);
  expect(onTransformPreview).toHaveBeenLastCalledWith({
    frameId: "frame-001",
    panX: -0.9,
    panY: 0,
    zoom: expect.any(Number),
  });
  expect(
    onTransformPreview.mock.calls[
      onTransformPreview.mock.calls.length - 1
    ]?.[0]?.zoom,
  ).toBeGreaterThan(1);
  expect(onZoomCommit).not.toHaveBeenCalled();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(300);
  });
  wheel();
  await act(async () => {
    await vi.advanceTimersByTimeAsync(300);
  });
  wheel();

  expect(onZoomCommit).not.toHaveBeenCalled();

  await act(async () => {
    await vi.advanceTimersByTimeAsync(500);
  });

  expect(onZoomCommit).toHaveBeenCalledOnce();
  expect(onZoomCommit.mock.calls[0][0]).toBe("frame-001");
  expect(onZoomCommit.mock.calls[0][1]).toBeGreaterThan(0);
});
