import { act, fireEvent, render, waitFor } from "@testing-library/react";
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
  displayWithHandler,
  finishPixiInitialization,
  getPixiLifecycle,
  latestDisplayWithHandler,
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
  const expectedScale = (500 - 2 * 24) / (300 + 24);

  expect(world.scale.x).toBeCloseTo(expectedScale, 4);
  expect(world.scale.y).toBeCloseTo(expectedScale, 4);
  expect(world.position.y - 24 * world.scale.y).toBeCloseTo(24, 4);
  expect(pixiLifecycle.initOptions[0]).toMatchObject({
    autoDensity: true,
    resolution: window.devicePixelRatio,
  });
});

test("shows only the resolved Photo target and drops only after a valid highlight", async () => {
  const onResolvePhotoDropTarget = vi.fn(async () => ({
    kind: "frame" as const,
    frameId: "frame-001",
  }));
  const onDropPhoto = vi.fn(async () => true);
  const onPhotoDragCancel = vi.fn();
  const view = renderCanvas({
    compositionPlan: interactiveComposition,
    draggedPhotoId: "media-002",
    onResolvePhotoDropTarget,
    onDropPhoto,
    onPhotoDragCancel,
  });
  await finishPixiInitialization();
  const canvas = pixiLifecycle.instances[0].canvas;
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    width: 1_200,
    height: 500,
    right: 1_200,
    bottom: 500,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  const host = view.container.querySelector(".canvas-host")!;
  const dataTransfer = { dropEffect: "none" };

  fireEvent.dragOver(host, { clientX: 600, clientY: 250, dataTransfer });
  await waitFor(() => {
    expect(displayWithLabel("frame-photo-drop-frame-001").visible).toBe(true);
  });
  expect(displayWithLabel("sheet-photo-drop-sheet-001").visible).toBe(false);
  expect(dataTransfer.dropEffect).toBe("copy");

  fireEvent.drop(host, { clientX: 600, clientY: 250, dataTransfer });
  await waitFor(() => expect(onDropPhoto).toHaveBeenCalledOnce());
  expect(onDropPhoto).toHaveBeenCalledWith(
    "media-002",
    expect.objectContaining({ sheetId: "sheet-001" }),
  );
  expect(onPhotoDragCancel).toHaveBeenCalledOnce();
});

test("Esc and an invalid Photo target cancel without a Project mutation", async () => {
  const onResolvePhotoDropTarget = vi.fn(async () => ({
    kind: "invalid" as const,
  }));
  const onDropPhoto = vi.fn(async () => true);
  const onPhotoDragCancel = vi.fn();
  const view = renderCanvas({
    compositionPlan: interactiveComposition,
    draggedPhotoId: "media-002",
    onResolvePhotoDropTarget,
    onDropPhoto,
    onPhotoDragCancel,
  });
  await finishPixiInitialization();
  const canvas = pixiLifecycle.instances[0].canvas;
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
    left: 0,
    top: 0,
    width: 1_200,
    height: 500,
    right: 1_200,
    bottom: 500,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });
  const host = view.container.querySelector(".canvas-host")!;
  const dataTransfer = { dropEffect: "none" };

  fireEvent.dragOver(host, { clientX: 600, clientY: 250, dataTransfer });
  await waitFor(() => expect(onResolvePhotoDropTarget).toHaveBeenCalled());
  fireEvent.drop(host, { clientX: 600, clientY: 250, dataTransfer });
  expect(onDropPhoto).not.toHaveBeenCalled();

  fireEvent.keyDown(window, { key: "Escape" });
  expect(onPhotoDragCancel).toHaveBeenCalled();
  expect(displayWithLabel("frame-photo-drop-frame-001").visible).toBe(false);
  expect(displayWithLabel("sheet-photo-drop-sheet-001").visible).toBe(false);
});

test("enters sheet editing by a double click on either surface or Frame", async () => {
  const onEnterSheetEdit = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onEnterSheetEdit,
  });
  await finishPixiInitialization();
  const sheet = displayWithHandler("pointertap");

  sheet.emit("pointertap", { target: sheet, detail: 2 });
  expect(onEnterSheetEdit).toHaveBeenCalledWith("sheet-001");

  latestDisplayWithHandler("pointertap").emit("pointertap", {
    detail: 2,
    stopPropagation: vi.fn(),
  });
  expect(onEnterSheetEdit).toHaveBeenCalledTimes(2);
  expect(onEnterSheetEdit).toHaveBeenLastCalledWith("sheet-001");
});

test("editing mode materializes only its isolated sheet", async () => {
  renderCanvas({
    compositionPlan: threeSheetComposition,
    editingSheetId: "sheet-002",
  });
  await finishPixiInitialization();
  const world = pixiLifecycle.instances[0].stage.children[0] as {
    children: unknown[];
  };

  expect(world.children).toHaveLength(1);
  expect(
    pixiLifecycle.displays.some(
      (display) => display.label === "sheet-focus-sheet-002",
    ),
  ).toBe(true);
  expect(
    pixiLifecycle.displays.some(
      (display) => display.label === "sheet-focus-sheet-001",
    ),
  ).toBe(false);
});

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
    },
  });
  await finishPixiInitialization();

  expect(
    displayWithLabel("frame-persisted-border-frame-001"),
  ).toBeDefined();
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
  const canvasScale = (500 - 2 * 24) / (300 + 24);
  const canvasAt = (focusedSheetId: string) => (
    <AlbumCanvas
      projectId="project-spike-001"
      composition={longComposition}
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
