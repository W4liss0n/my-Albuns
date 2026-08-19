import { act, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import type { LogEvent, Logger } from "../application/logging";
import type { CompositionPlan } from "../domain/project";
import {
  composition,
  interactiveComposition,
  threeSheetComposition,
} from "./albumCanvasTestFixtures";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import {
  AlbumCanvas,
  displayWithLabel,
  finishPixiInitialization,
  getPixiLifecycle,
  renderCanvas,
  setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

setupAlbumCanvasTestHarness();
const pixiLifecycle = getPixiLifecycle();

test("fits the complete sheet to the continuous Canvas at device resolution", async () => {
  renderCanvas();
  await finishPixiInitialization();

  const world = pixiLifecycle.instances[0].stage.children[0] as {
    position: { y: number };
    scale: { x: number; y: number };
  };
  const expectedScale = (500 - 2 * 28) / 300;

  expect(world.scale.x).toBeCloseTo(expectedScale, 4);
  expect(world.scale.y).toBeCloseTo(expectedScale, 4);
  expect(world.position.y).toBeCloseTo(28, 4);
  expect(pixiLifecycle.initOptions[0]).toMatchObject({
    autoDensity: true,
    resolution: window.devicePixelRatio,
  });
});

test("matches the reference Canvas surface, technical guides and empty Frame treatment", async () => {
  const placeholderComposition: CompositionPlan = {
    ...composition,
    sheets: composition.sheets.map((sheet) => ({
      ...sheet,
      frames: [
        {
          borderFillRects: [],
          clipRect: {
            x: 24_000,
            y: 24_000,
            width: 252_000,
            height: 252_000,
          },
          frameId: "frame-placeholder",
          photo: null,
          zIndex: 0,
        },
      ],
    })),
  };
  renderCanvas({
    compositionPlan: placeholderComposition,
    technicalGuides: { bleedUm: 3_000, safetyUm: 5_000 },
  });
  await finishPixiInitialization();

  const surface = displayWithLabel("sheet-surface-sheet-001") as unknown as {
    rectCommands: Array<{
      height: number;
      width: number;
      x: number;
      y: number;
    }>;
    strokeStyles: unknown[];
  };
  expect(surface.rectCommands).toEqual([
    { height: 300, width: 600, x: 0, y: 0 },
  ]);
  expect(surface.strokeStyles).toContainEqual(
    expect.objectContaining({ pixelLine: true, width: 1 }),
  );

  const closeShadow = displayWithLabel(
    "sheet-shadow-close-sheet-001",
  ) as unknown as {
    filters: unknown[];
    rectCommands: unknown[];
  };
  const depthShadow = displayWithLabel(
    "sheet-shadow-depth-sheet-001",
  ) as unknown as {
    filters: unknown[];
    rectCommands: unknown[];
  };
  expect(closeShadow.filters).toEqual([]);
  expect(depthShadow.filters).toEqual([]);
  expect(closeShadow.rectCommands.length).toBeLessThan(
    depthShadow.rectCommands.length,
  );

  const centerLine = displayWithLabel(
    "sheet-center-line-sheet-001",
  ) as unknown as { strokeStyles: Array<{ color: number }> };
  expect(centerLine.strokeStyles).toContainEqual(
    expect.objectContaining({ color: 0xeeeae1 }),
  );

  const placeholderBase = displayWithLabel(
    "frame-placeholder-base-frame-placeholder",
  ) as unknown as { fillStyles: Array<{ color: number }> };
  const placeholderStripes = displayWithLabel(
    "frame-placeholder-stripes-frame-placeholder",
  ) as unknown as {
    fillStyles: Array<{ color: number }>;
    pathCommands: unknown[];
  };
  expect(placeholderBase.fillStyles).toContainEqual(
    expect.objectContaining({ color: 0xe6e1d7 }),
  );
  expect(placeholderStripes.fillStyles).toContainEqual(
    expect.objectContaining({ color: 0xdcd6ca }),
  );
  expect(placeholderStripes.pathCommands.length).toBeGreaterThan(0);
  expect(
    getPixiLifecycle().displays.some(
      ({ label }) => label === "frame-placeholder-cross-frame-placeholder",
    ),
  ).toBe(false);

  expect(displayWithLabel("sheet-bleed-guide-sheet-001")).toBeDefined();
  expect(displayWithLabel("sheet-safety-guide-sheet-001")).toBeDefined();
  expect(displayWithLabel("sheet-bleed-mask-sheet-001")).toMatchObject({
    rectCommands: [
      { height: 3, width: 600, x: 0, y: 0 },
      { height: 294, width: 3, x: 597, y: 3 },
      { height: 3, width: 600, x: 0, y: 297 },
      { height: 294, width: 3, x: 0, y: 3 },
    ],
  });
});

test.each([
  {
    guides: { bleedUm: 0, safetyUm: 5_000 },
    expectedLabels: ["sheet-safety-guide-sheet-001"],
  },
  {
    guides: { bleedUm: 3_000, safetyUm: 0 },
    expectedLabels: [
      "sheet-bleed-mask-sheet-001",
      "sheet-bleed-guide-sheet-001",
    ],
  },
  { guides: { bleedUm: 0, safetyUm: 0 }, expectedLabels: [] },
])(
  "disables zero-valued technical guides independently ($guides)",
  async ({ guides, expectedLabels }) => {
    renderCanvas({ technicalGuides: guides });
    await finishPixiInitialization();

    const technicalLabels = pixiLifecycle.displays
      .map(({ label }) => label)
      .filter((label) =>
        [
          "sheet-bleed-guide-sheet-001",
          "sheet-safety-guide-sheet-001",
          "sheet-bleed-mask-sheet-001",
        ].includes(label),
      );
    expect(technicalLabels).toEqual(expectedLabels);
  },
);

test.each([
  {
    activeSides: "right" as const,
    omittedGuideX: 3,
    inactiveBoundaryX: 0,
    omittedMaskX: 0,
    retainedGuideX: 297,
    retainedMaskX: 297,
  },
  {
    activeSides: "left" as const,
    omittedGuideX: 297,
    inactiveBoundaryX: 300,
    omittedMaskX: 297,
    retainedGuideX: 3,
    retainedMaskX: 0,
  },
])(
  "omits guides and bleed mask on the inactive-side edge of a $activeSides single Page",
  async ({
    activeSides,
    inactiveBoundaryX,
    omittedGuideX,
    omittedMaskX,
    retainedGuideX,
    retainedMaskX,
  }) => {
    renderCanvas({
      compositionPlan: createSinglePageComposition(activeSides),
      sheetBarMetadata: [{ sheetId: "sheet-001", pageNumbers: [1] }],
      technicalGuides: { bleedUm: 3_000, safetyUm: 5_000 },
    });
    await finishPixiInitialization();

    const guide = displayWithLabel(
      "sheet-bleed-guide-sheet-001",
    ) as unknown as {
      pathCommands: Array<{
        kind: "lineTo" | "moveTo";
        x: number;
        y: number;
      }>;
    };
    expect(hasVerticalSegmentAt(guide.pathCommands, omittedGuideX)).toBe(
      false,
    );
    expect(hasVerticalSegmentAt(guide.pathCommands, retainedGuideX)).toBe(
      true,
    );
    expect(
      guide.pathCommands.some(({ x }) => x === inactiveBoundaryX),
    ).toBe(true);

    const bleedMask = displayWithLabel(
      "sheet-bleed-mask-sheet-001",
    ) as unknown as {
      rectCommands: Array<{
        height: number;
        width: number;
        x: number;
        y: number;
      }>;
    };
    expect(
      bleedMask.rectCommands.some(
        ({ height, width, x }) =>
          x === omittedMaskX && width === 3 && height === 294,
      ),
    ).toBe(false);
    expect(
      bleedMask.rectCommands.some(
        ({ height, width, x }) =>
          x === retainedMaskX && width === 3 && height === 294,
      ),
    ).toBe(true);
  },
);

test("materializes the integrated Sheet Bar instead of a loose sheet label", async () => {
  vi.useFakeTimers();
  renderCanvas();
  await finishPixiInitialization();

  expect(displayWithLabel("sheet-bar-surface-sheet-001")).toMatchObject({
    rectCommands: [{ height: 40, width: 600, x: 0, y: 0 }],
  });
  expect(displayWithLabel("sheet-bar-page-left-sheet-001")).toMatchObject({
    text: "1",
  });
  expect(displayWithLabel("sheet-bar-page-right-sheet-001")).toMatchObject({
    text: "2",
  });
  const swapAction = displayWithLabel(
    "placeholder-sheet-bar-swap-sheet-001",
  );
  const layoutAction = displayWithLabel(
    "placeholder-sheet-bar-layout-sheet-001",
  );
  expect(swapAction).toMatchObject({
    alpha: 0.45,
    eventMode: "static",
    tint: 0x6d675d,
  });
  expect(layoutAction).toMatchObject({
    alpha: 0.45,
    eventMode: "static",
    tint: 0x6d675d,
  });
  expect(
    screen.getByRole("button", {
      name: "Trocar Frames — indisponível nesta versão",
    }),
  ).toBeDisabled();
  expect(
    screen.getByRole("button", {
      name: "Abrir Painel de Layouts — indisponível nesta versão",
    }),
  ).toBeDisabled();
  expect(displayWithLabel("sheet-bar-number-sheet-001")).toMatchObject({
    text: "L01",
  });

  const sheetBar = displayWithLabel("sheet-bar-sheet-001");
  const sheet = displayWithLabel("canvas-sheet-sheet-001");
  expect(sheetBar.alpha).toBe(0);
  expect(sheetBar.scale.y).toBeCloseTo(1 / ((500 - 2 * 28) / 300));

  sheet.emit("pointerenter", {});
  expect(sheetBar.alpha).toBe(0);
  await act(async () => {
    await vi.advanceTimersByTimeAsync(160);
  });
  expect(sheetBar.alpha).toBe(0.55);

  sheetBar.emit("pointerenter", {});
  await act(async () => {
    await vi.advanceTimersByTimeAsync(160);
  });
  expect(sheetBar.alpha).toBe(1);

  for (const action of [swapAction, layoutAction]) {
    action.emit("pointerenter", {});
    expect(action).toMatchObject({ alpha: 1, tint: 0x2c2924 });
    action.emit("pointerleave", {});
    expect(action).toMatchObject({ alpha: 0.45, tint: 0x6d675d });
  }

  sheetBar.emit("pointerleave", {});
  await act(async () => {
    await vi.advanceTimersByTimeAsync(160);
  });
  expect(sheetBar.alpha).toBe(0.55);

  sheet.emit("pointerleave", {});
  await act(async () => {
    await vi.advanceTimersByTimeAsync(160);
  });
  expect(sheetBar.alpha).toBe(0);
});

test("centers the canonical page number on a single-page sheet bar", async () => {
  renderCanvas({
    compositionPlan: createSinglePageComposition("right"),
    sheetBarMetadata: [{ sheetId: "sheet-001", pageNumbers: [1] }],
  });
  await finishPixiInitialization();

  expect(displayWithLabel("sheet-bar-page-right-sheet-001")).toMatchObject({
    position: { x: 150, y: 20 },
    text: "1",
  });
  expect(
    pixiLifecycle.displays.some(
      ({ label }) => label === "sheet-bar-page-left-sheet-001",
    ),
  ).toBe(false);
});

function createSinglePageComposition(
  activeSides: "left" | "right",
): CompositionPlan {
  return {
    ...composition,
    sheets: [
      {
        ...composition.sheets[0],
        activeSides,
        widthUm: 300_000,
        base: {
          ...composition.sheets[0].base,
          drawRect: {
            ...composition.sheets[0].base.drawRect,
            width: 300_000,
          },
        },
        backgrounds: composition.sheets[0].backgrounds.map((background) => ({
          ...background,
          drawRect: { ...background.drawRect, width: 300_000 },
        })),
      },
    ],
  };
}

function hasVerticalSegmentAt(
  commands: Array<{
    kind: "lineTo" | "moveTo";
    x: number;
    y: number;
  }>,
  x: number,
) {
  return commands.some((command, index) => {
    const next = commands[index + 1];
    return (
      command.kind === "moveTo" &&
      next?.kind === "lineTo" &&
      command.x === x &&
      next.x === x &&
      command.y !== next.y
    );
  });
}

test("keeps the materialized Pixi scene stable across view-only updates", async () => {
  const callbacks = {
    onSelectFrame: vi.fn(),
    onFocusSheet: vi.fn(),
    onCenteredSheetChange: vi.fn(),
    onViewportChange: vi.fn(),
    onTransformPreview: vi.fn(),
    onTransformCommit: vi.fn(async () => true),
  };
  const layout = createContinuousCanvasLayout(
    interactiveComposition.sheets,
  );
  const canvas = (
    selectedFrameId: string | null,
    offsetX: number,
  ) => (
    <AlbumCanvas
      projectId="project-spike-001"
      composition={interactiveComposition}
      sheetBarMetadata={[]}
      continuousCanvasLayout={layout}
      selectedFrameId={selectedFrameId}
      focusedSheetId="sheet-001"
      centeredSheetId="sheet-001"
      viewport={{ offsetX }}
      {...callbacks}
    />
  );

  const view = render(canvas(null, 42));
  await finishPixiInitialization();
  const world = pixiLifecycle.instances[0].stage.children[0] as {
    children: unknown[];
  };
  const sheet = world.children[0];
  const displayCount = pixiLifecycle.displays.length;

  view.rerender(canvas("frame-001", 35));

  expect(pixiLifecycle.instances[0].stage.children[0]).toBe(world);
  expect(world.children[0]).toBe(sheet);
  expect(pixiLifecycle.displays).toHaveLength(displayCount);
});

test("materializes a reduced Cache preview as the Canvas texture", async () => {
  const texture = { label: "cache-preview" };
  const logEvents: LogEvent[] = [];
  const logger: Logger = {
    write: (event) => logEvents.push(event),
  };
  renderCanvas({
    compositionPlan: interactiveComposition,
    mediaPreviewUrls: {
      "media-001": "http://myalbuns-cache.localhost/photo-token",
    },
    logger,
  });
  await finishPixiInitialization();

  expect(pixiLifecycle.assetLoads).toEqual([
    "http://myalbuns-cache.localhost/photo-token",
  ]);
  expect(pixiLifecycle.spriteTextures).toEqual([]);

  await act(async () => {
    pixiLifecycle.resolveAssetLoads[0]?.(texture);
    await Promise.resolve();
    await Promise.resolve();
  });
  await waitFor(() => {
    expect(pixiLifecycle.spriteTextures).toEqual([texture, texture]);
  });
  expect(logEvents).toEqual(
    expect.arrayContaining([
      expect.objectContaining({
        component: "canvas",
        event: "canvas_opaque_preview_texture_loaded",
      }),
    ]),
  );
  expect(
    logEvents.filter(
      ({ event }) => event === "canvas_opaque_preview_texture_loaded",
    ),
  ).toHaveLength(1);
});

test("materializes a transparent Decorative from the shared Cache URL", async () => {
  const previewUrl =
    "http://myalbuns-cache.localhost/decorative-token";
  const texture = { label: "decorative-cache-preview" };
  const decorativeComposition: CompositionPlan = {
    ...composition,
    sheets: [
      {
        ...composition.sheets[0],
        overlays: [
          {
            mediaId: "decorative-overlay",
            name: "Overlay translúcido.png",
            drawRect: {
              x: 0,
              y: 0,
              width: composition.sheets[0].widthUm,
              height: composition.sheets[0].heightUm,
            },
          },
        ],
      },
    ],
  };

  renderCanvas({
    compositionPlan: decorativeComposition,
    mediaPreviewUrls: {
      "decorative-overlay": previewUrl,
    },
  });
  await finishPixiInitialization();

  expect(pixiLifecycle.assetLoads).toEqual([previewUrl]);

  await act(async () => {
    pixiLifecycle.resolveAssetLoads[0]?.(texture);
    await Promise.resolve();
    await Promise.resolve();
  });
  await waitFor(() => {
    expect(pixiLifecycle.spriteTextures).toEqual([texture]);
    expect(
      displayWithLabel("decorative-overlay-decorative-overlay"),
    ).toBeDefined();
  });
});

test("renders Background and Overlay from opaque Cache representations and reports visible demand", async () => {
  const backgroundTexture = { label: "background-cache-preview" };
  const overlayTexture = { label: "overlay-cache-preview" };
  const onMediaDemandChange = vi.fn();
  const faithfulComposition: CompositionPlan = {
    ...composition,
    sheets: [
      {
        ...composition.sheets[0],
        backgrounds: [
          {
            kind: "media",
            mediaId: "background-media",
            name: "Background.jpg",
            drawRect: {
              x: 0,
              y: 0,
              width: composition.sheets[0].widthUm,
              height: composition.sheets[0].heightUm,
            },
          },
        ],
        overlays: [
          {
            mediaId: "overlay-media",
            name: "Overlay.png",
            drawRect: {
              x: 0,
              y: 0,
              width: composition.sheets[0].widthUm,
              height: composition.sheets[0].heightUm,
            },
          },
        ],
      },
    ],
  };

  renderCanvas({
    compositionPlan: faithfulComposition,
    mediaPreviewUrls: {
      "background-media":
        "http://myalbuns-cache.localhost/background-token",
      "overlay-media": "http://myalbuns-cache.localhost/overlay-token",
    },
    onMediaDemandChange,
  });
  await finishPixiInitialization();

  expect(onMediaDemandChange).toHaveBeenLastCalledWith({
    visibleMediaIds: ["background-media", "overlay-media"],
    preloadMediaIds: [],
  });
  expect(pixiLifecycle.assetLoads).toEqual([
    "http://myalbuns-cache.localhost/background-token",
    "http://myalbuns-cache.localhost/overlay-token",
  ]);

  await act(async () => {
    pixiLifecycle.resolveAssetLoads[0]?.(backgroundTexture);
    pixiLifecycle.resolveAssetLoads[1]?.(overlayTexture);
    await Promise.resolve();
    await Promise.resolve();
  });
  await waitFor(() => {
    expect(displayWithLabel("background-media-background-media"))
      .toBeDefined();
    expect(displayWithLabel("decorative-overlay-overlay-media"))
      .toBeDefined();
  });
});

test("materializes the persisted Frame border in the Canvas scene", async () => {
  renderCanvas({
    compositionPlan: {
      ...interactiveComposition,
      frameBorder: {
        kind: "solid",
        rgb: "#A0B0C0",
        widthUm: 1_250,
      },
      sheets: interactiveComposition.sheets.map((sheet) => ({
        ...sheet,
        frames: sheet.frames.map((frame) => ({
          ...frame,
          borderFillRects: [
            { x: 0, y: 0, width: 300_000, height: 1_250 },
            { x: 0, y: 198_750, width: 300_000, height: 1_250 },
            { x: 0, y: 0, width: 1_250, height: 200_000 },
            { x: 298_750, y: 0, width: 1_250, height: 200_000 },
          ],
        })),
      })),
    },
  });
  await finishPixiInitialization();

  const border = displayWithLabel(
    "frame-persisted-border-frame-001",
  ) as unknown as {
    rectCommands: Array<{
      height: number;
      width: number;
      x: number;
      y: number;
    }>;
    fillStyles: unknown[];
  };
  expect(border.rectCommands).toEqual([
    { height: 1.25, width: 300, x: 0, y: 0 },
    { height: 1.25, width: 300, x: 0, y: 198.75 },
    { height: 200, width: 1.25, x: 0, y: 0 },
    { height: 200, width: 1.25, x: 298.75, y: 0 },
  ]);
  expect(border.fillStyles).toContainEqual(
    expect.objectContaining({ color: 0xa0b0c0 }),
  );
});

test("materializes and releases only the viewport margin while navigating a long Album", async () => {
  const longComposition: CompositionPlan = {
    ...interactiveComposition,
    sheets: Array.from({ length: 100 }, (_, index) => {
      const number = index + 1;
      return {
        ...interactiveComposition.sheets[0],
        sheetId: `sheet-${String(number).padStart(3, "0")}`,
        number,
        overlays: [
          {
            mediaId: "decorative-overlay",
            name: "Overlay translúcido.png",
            drawRect: {
              x: 0,
              y: 0,
              width: 600_000,
              height: 300_000,
            },
          },
        ],
        frames: [
          {
            ...interactiveComposition.sheets[0].frames[0],
            frameId: `frame-${String(number).padStart(3, "0")}`,
            photo: {
              ...interactiveComposition.sheets[0].frames[0].photo!,
              mediaId: `media-${String(number).padStart(3, "0")}`,
              name: `Foto ${number}.jpg`,
            },
          },
        ],
      };
    }),
  };
  const mediaPreviewUrls = Object.fromEntries(
    [
      ...longComposition.sheets.map((sheet) => {
        const mediaId = sheet.frames[0].photo!.mediaId;
        return [
          mediaId,
          `asset://localhost/cache/${mediaId}.jpg`,
        ];
      }),
      [
        "decorative-overlay",
        "asset://localhost/cache/decorative-overlay.png",
      ],
    ],
  );
  const callbacks = {
    onSelectFrame: vi.fn(),
    onFocusSheet: vi.fn(),
    onCenteredSheetChange: vi.fn(),
    onViewportChange: vi.fn(),
    onTransformPreview: vi.fn(),
    onTransformCommit: vi.fn(async () => true),
  };
  const layout = createContinuousCanvasLayout(longComposition.sheets);
  const canvasScale = (500 - 2 * 28) / 300;
  const canvasAt = (focusedSheetId: string) => (
    <AlbumCanvas
      projectId="project-spike-001"
      composition={longComposition}
      sheetBarMetadata={[]}
      mediaPreviewUrls={mediaPreviewUrls}
      continuousCanvasLayout={layout}
      selectedFrameId={null}
      focusedSheetId={focusedSheetId}
      centeredSheetId={focusedSheetId}
      viewport={{
        offsetX:
          layout.centeredOffset(focusedSheetId, canvasScale, 1_200) ?? 0,
      }}
      {...callbacks}
    />
  );
  let settledLoadCount = 0;
  const settleNewTextures = async () => {
    await act(async () => {
      while (settledLoadCount < pixiLifecycle.resolveAssetLoads.length) {
        pixiLifecycle.resolveAssetLoads[settledLoadCount]?.({
          label: `viewport-texture-${settledLoadCount}`,
        });
        settledLoadCount += 1;
      }
      await Promise.resolve();
      await Promise.resolve();
    });
  };

  const view = render(canvasAt("sheet-001"));
  await finishPixiInitialization();

  const world = pixiLifecycle.instances[0].stage.children[0] as {
    children: Array<{ position: { x: number } }>;
  };
  const initialSheets = [...world.children];
  const firstPreview = mediaPreviewUrls["media-001"];
  const middlePreview = mediaPreviewUrls["media-050"];
  const lastPreview = mediaPreviewUrls["media-100"];

  expect(longComposition.sheets).toHaveLength(100);
  expect(initialSheets.length).toBeGreaterThan(0);
  expect(initialSheets.length).toBeLessThanOrEqual(8);
  expect(pixiLifecycle.assetLoads).toContain(firstPreview);
  expect(pixiLifecycle.assetLoads).not.toContain(lastPreview);
  await settleNewTextures();

  view.rerender(canvasAt("sheet-050"));

  expect(world.children.length).toBeGreaterThan(0);
  expect(world.children.length).toBeLessThanOrEqual(8);
  expect(world.children).not.toContain(initialSheets[0]);
  expect(pixiLifecycle.assetLoads).toContain(middlePreview);
  await waitFor(() => {
    expect(pixiLifecycle.assetUnloads).toContain(firstPreview);
  });
  await settleNewTextures();

  view.rerender(canvasAt("sheet-100"));

  expect(world.children.length).toBeGreaterThan(0);
  expect(world.children.length).toBeLessThanOrEqual(8);
  expect(pixiLifecycle.assetLoads).toContain(lastPreview);
  await waitFor(() => {
    expect(pixiLifecycle.assetUnloads).toContain(middlePreview);
  });
  await settleNewTextures();

  view.rerender(canvasAt("sheet-001"));

  expect(world.children.length).toBeGreaterThan(0);
  expect(world.children.length).toBeLessThanOrEqual(8);
  expect(world.children.some(({ position }) => position.x === 0)).toBe(true);
  expect(world.children).not.toContain(initialSheets[0]);
  expect(
    pixiLifecycle.assetLoads.filter((url) => url === firstPreview),
  ).toHaveLength(2);
  await waitFor(() => {
    expect(pixiLifecycle.assetUnloads).toContain(lastPreview);
  });
});

test("reconciles only the composed sheet that changed", async () => {
  const canvasProps = {
    projectId: "project-spike-001",
    sheetBarMetadata: [],
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 42 },
    onSelectFrame: vi.fn(),
    onFocusSheet: vi.fn(),
    onCenteredSheetChange: vi.fn(),
    onViewportChange: vi.fn(),
    onTransformPreview: vi.fn(),
    onTransformCommit: vi.fn(async () => true),
  };
  const view = render(
    <AlbumCanvas
      composition={threeSheetComposition}
      continuousCanvasLayout={createContinuousCanvasLayout(
        threeSheetComposition.sheets,
      )}
      {...canvasProps}
    />,
  );
  await finishPixiInitialization();

  const world = pixiLifecycle.instances[0].stage.children[0] as {
    children: Array<{ position: { x: number } }>;
  };
  const originalSheets = [...world.children].sort(
    (first, second) => first.position.x - second.position.x,
  );
  const changedComposition: CompositionPlan = {
    ...threeSheetComposition,
    sheets: threeSheetComposition.sheets.map((sheet, index) =>
      index === 1
        ? {
            ...sheet,
            frames: [
              {
                frameId: "frame-002",
                clipRect: {
                  x: 20_000,
                  y: 20_000,
                  width: 200_000,
                  height: 200_000,
                },
                borderFillRects: [],
                zIndex: 0,
                photo: null,
              },
            ],
          }
        : sheet,
    ),
  };

  view.rerender(
    <AlbumCanvas
      composition={changedComposition}
      continuousCanvasLayout={createContinuousCanvasLayout(
        changedComposition.sheets,
      )}
      {...canvasProps}
    />,
  );

  const reconciledSheets = [...world.children].sort(
    (first, second) => first.position.x - second.position.x,
  );
  expect(pixiLifecycle.instances).toHaveLength(1);
  expect(reconciledSheets[0]).toBe(originalSheets[0]);
  expect(reconciledSheets[1]).not.toBe(originalSheets[1]);
  expect(reconciledSheets[2]).toBe(originalSheets[2]);
});
