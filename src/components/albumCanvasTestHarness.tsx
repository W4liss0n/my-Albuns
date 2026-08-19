import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, vi } from "vitest";

import type { CompositionPlan } from "../domain/project";
import type { MediaPreviewDemand } from "../application/projectPorts";
import {
  AlbumCanvas as ProductionAlbumCanvas,
  type AlbumCanvasMode,
  type AlbumCanvasProps,
  type CanvasMetrics,
  type CanvasTechnicalGuides,
  type PhotoTransformDelta,
  type PhotoTransformPreview,
  type SheetBarMetadata,
} from "./AlbumCanvas";
import type { Logger } from "../application/logging";
import { silentLogger } from "../application/logging";
import type { GraphicsDiagnostic } from "../application/graphics";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import { createNormalCanvasLayout } from "./canvasSheetViewGeometry";
import {
  CanvasGraphicsDiagnosticProbeProvider,
  type CanvasGraphicsDiagnosticProbe,
} from "./canvasGraphicsDiagnosticProbeContext";
import { composition } from "./albumCanvasTestFixtures";
import { LoggingProvider } from "./loggingContext";

const pixiLifecycle = vi.hoisted(() => ({
  displays: [] as Array<{
    alpha: number;
    children: unknown[];
    cursor: string;
    eventMode: string;
    filters: unknown[];
    handlers: Map<string, (event: unknown) => void>;
    hitArea: unknown;
    label: string;
    mask: unknown;
    pathCommands: Array<{
      kind: "lineTo" | "moveTo";
      x: number;
      y: number;
    }>;
    position: { x: number; y: number };
    scale: { x: number; y: number };
    style?: Record<string, unknown>;
    text?: string;
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
  assetLoads: [] as string[],
  assetUnloads: [] as string[],
  resolveAssetLoads: [] as Array<(texture: object) => void>,
  rejectAssetLoads: [] as Array<(reason?: unknown) => void>,
  spriteTextures: [] as unknown[],
  fillGradients: [] as Array<{ destroyCount: number }>,
}));

export function getPixiLifecycle() {
  return pixiLifecycle;
}

const availableCanvasGraphicsDiagnosticProbe: CanvasGraphicsDiagnosticProbe =
  () => ({
    supported: true,
    renderer: "ANGLE (NVIDIA GeForce RTX 3050)",
    reason: "WebGL2 acelerado por hardware confirmado.",
    limits: {
      maxTextureSizePx: 16_384,
      maxRenderbufferSizePx: 16_384,
      maxTextureImageUnits: 16,
    },
  });

export function AlbumCanvas({
  canvasGraphicsDiagnosticProbe =
    availableCanvasGraphicsDiagnosticProbe,
  ...props
}: AlbumCanvasProps & {
  canvasGraphicsDiagnosticProbe?: CanvasGraphicsDiagnosticProbe;
}) {
  return (
    <CanvasGraphicsDiagnosticProbeProvider
      probe={canvasGraphicsDiagnosticProbe}
    >
      <ProductionAlbumCanvas {...props} />
    </CanvasGraphicsDiagnosticProbeProvider>
  );
}

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
    filters: unknown[] = [];
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
    tint = 0xffffff;
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

    removeChild(...children: DisplayObject[]) {
      children.forEach((child) => {
        const index = this.children.indexOf(child);
        if (index >= 0) this.children.splice(index, 1);
        child.parent = null;
      });
      return children[0];
    }
  }

  class Container extends DisplayObject {}

  class FillGradient {
    colorStops: unknown;
    destroyCount = 0;
    end: unknown;
    start: unknown;
    type: unknown;

    constructor(options: Record<string, unknown>) {
      this.colorStops = options.colorStops;
      this.end = options.end;
      this.start = options.start;
      this.type = options.type;
      pixiLifecycle.fillGradients.push(this);
    }

    destroy() {
      this.destroyCount += 1;
    }
  }

  class Graphics extends DisplayObject {
    fillStyles: unknown[] = [];
    rectCommands: Array<{
      height: number;
      width: number;
      x: number;
      y: number;
    }> = [];
    strokeStyles: unknown[] = [];

    circle() {
      return this;
    }

    fill(style?: unknown) {
      this.fillStyles.push(style);
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

    rect(x: number, y: number, width: number, height: number) {
      this.rectCommands.push({ height, width, x, y });
      return this;
    }

    roundRect() {
      return this;
    }

    stroke(style?: unknown) {
      this.strokeStyles.push(style);
      return this;
    }
  }

  class Text extends DisplayObject {
    anchor = new Point();
    style: Record<string, unknown>;
    text: string;

    constructor(options: {
      style?: Record<string, unknown>;
      text?: string;
    } = {}) {
      super();
      this.style = options.style ?? {};
      this.text = options.text ?? "";
    }
  }

  class Sprite extends DisplayObject {
    height = 0;
    width = 0;
    texture: unknown;

    constructor(options: {
      texture?: unknown;
      width?: number;
      height?: number;
    } = {}) {
      super();
      this.texture = options.texture;
      this.width = options.width ?? 0;
      this.height = options.height ?? 0;
      if (this.texture !== undefined) {
        pixiLifecycle.spriteTextures.push(this.texture);
      }
    }
  }

  class Application {
    canvas = document.createElement("canvas");
    destroyCount = 0;
    initialized = false;
    resizeCount = 0;
    resizeTarget: HTMLElement | null = null;
    screen = { width: 1_200, height: 500 };
    stage = new Container();
    ticker = {
      addOnce: (callback: () => void) => callback(),
    };

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
    Assets: {
      setPreferences: vi.fn(),
      load: vi.fn(
        (url: string) =>
          new Promise<object>((resolve, reject) => {
            pixiLifecycle.assetLoads.push(url);
            pixiLifecycle.resolveAssetLoads.push(resolve);
            pixiLifecycle.rejectAssetLoads.push(reject);
          }),
      ),
      unload: vi.fn(async (url: string) => {
        pixiLifecycle.assetUnloads.push(url);
      }),
    },
    Container,
    FederatedPointerEvent: class {},
    FederatedWheelEvent: class {},
    FillGradient,
    Graphics,
    Rectangle: class {
      constructor(
        readonly x = 0,
        readonly y = 0,
        readonly width = 0,
        readonly height = 0,
      ) {}
    },
    Sprite,
    Text,
  };
});


export function renderCanvas({
  projectId = "project-spike-001",
  mode = { kind: "normal" },
  compositionPlan = composition,
  sheetBarMetadata = [
    { sheetId: "sheet-001", pageNumbers: [1, 2] },
  ],
  mediaPreviewUrls,
  technicalGuides,
  onCanvasMetricsChange = vi.fn<(metrics: CanvasMetrics) => void>(),
  onEditSheet = vi.fn<(sheetId: string) => void>(),
  onFocusSheet = vi.fn<(sheetId: string) => void>(),
  onCenteredSheetChange = vi.fn<(sheetId: string) => void>(),
  onTransformPreview = vi.fn<
    (preview: PhotoTransformPreview | null) => void
  >(),
  onViewportChange = vi.fn<(viewport: { offsetX: number }) => void>(),
  onTransformCommit = vi.fn(
    async (_delta: PhotoTransformDelta) => true,
  ),
  onMediaDemandChange,
  onGraphicsUnavailable,
  canvasGraphicsDiagnosticProbe,
  logger = silentLogger,
}: {
  projectId?: string;
  mode?: AlbumCanvasMode;
  compositionPlan?: CompositionPlan;
  sheetBarMetadata?: readonly SheetBarMetadata[];
  mediaPreviewUrls?: Readonly<Record<string, string>>;
  technicalGuides?: CanvasTechnicalGuides;
  onCanvasMetricsChange?: (metrics: CanvasMetrics) => void;
  onEditSheet?: (sheetId: string) => void;
  onFocusSheet?: (sheetId: string) => void;
  onCenteredSheetChange?: (sheetId: string) => void;
  onTransformPreview?: (
    preview: PhotoTransformPreview | null,
  ) => void;
  onViewportChange?: (viewport: { offsetX: number }) => void;
  onTransformCommit?: (
    delta: PhotoTransformDelta,
  ) => Promise<boolean>;
  onMediaDemandChange?: (demand: MediaPreviewDemand) => void;
  onGraphicsUnavailable?: (diagnostic: GraphicsDiagnostic) => void;
  canvasGraphicsDiagnosticProbe?: CanvasGraphicsDiagnosticProbe;
  logger?: Logger;
} = {}) {
  const view = render(
    <LoggingProvider logger={logger}>
      <AlbumCanvas
        canvasGraphicsDiagnosticProbe={
          canvasGraphicsDiagnosticProbe
        }
        projectId={projectId}
        mode={mode}
        composition={compositionPlan}
        sheetBarMetadata={sheetBarMetadata}
        mediaPreviewUrls={mediaPreviewUrls}
        technicalGuides={technicalGuides}
        continuousCanvasLayout={
          mode.kind === "normal"
            ? createNormalCanvasLayout(
                compositionPlan.sheets,
                technicalGuides?.bleedUm,
              )
            : createContinuousCanvasLayout(compositionPlan.sheets)
        }
        selectedFrameId={null}
        focusedSheetId="sheet-001"
        centeredSheetId="sheet-001"
        viewport={{ offsetX: 42 }}
        onSelectFrame={() => undefined}
        onEditSheet={onEditSheet}
        onFocusSheet={onFocusSheet}
        onCenteredSheetChange={onCenteredSheetChange}
        onViewportChange={onViewportChange}
        onTransformPreview={onTransformPreview}
        onTransformCommit={onTransformCommit}
        onCanvasMetricsChange={onCanvasMetricsChange}
        onMediaDemandChange={onMediaDemandChange}
        onGraphicsUnavailable={onGraphicsUnavailable}
      />
    </LoggingProvider>,
  );

  return {
    ...view,
    onCanvasMetricsChange,
    onEditSheet,
    onFocusSheet,
    onCenteredSheetChange,
    onTransformPreview,
    onViewportChange,
    onTransformCommit,
  };
}

export async function finishPixiInitialization() {
  await act(async () => {
    pixiLifecycle.resolveInitializations[
      pixiLifecycle.resolveInitializations.length - 1
    ]?.();
    await Promise.resolve();
    await Promise.resolve();
  });
}

export function displayWithHandler(name: string) {
  const display = pixiLifecycle.displays.find((candidate) =>
    candidate.handlers.has(name),
  );
  if (!display) throw new Error(`Pixi display with ${name} handler not found`);
  return display;
}

export function latestDisplayWithHandler(name: string) {
  const displays = pixiLifecycle.displays.filter((candidate) =>
    candidate.handlers.has(name),
  );
  const display = displays[displays.length - 1];
  if (!display) {
    throw new Error(`Pixi display with ${name} handler not found`);
  }
  return display;
}

export function displayWithLabel(label: string) {
  const display = pixiLifecycle.displays.find(
    (candidate) => candidate.label === label,
  );
  if (!display) throw new Error(`Pixi display labeled ${label} not found`);
  return display;
}

export function setupAlbumCanvasTestHarness() {
  beforeEach(() => {
    pixiLifecycle.displays.length = 0;
    pixiLifecycle.initOptions.length = 0;
    pixiLifecycle.instances.length = 0;
    pixiLifecycle.resizeCallbacks.length = 0;
    pixiLifecycle.resolveInitializations.length = 0;
    pixiLifecycle.assetLoads.length = 0;
    pixiLifecycle.assetUnloads.length = 0;
    pixiLifecycle.resolveAssetLoads.length = 0;
    pixiLifecycle.rejectAssetLoads.length = 0;
    pixiLifecycle.spriteTextures.length = 0;
    pixiLifecycle.fillGradients.length = 0;
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
    vi.restoreAllMocks();
  });
}
