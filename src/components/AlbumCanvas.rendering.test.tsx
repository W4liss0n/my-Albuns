import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import finalRendererCorpus from "../../tests/fixtures/final-renderer-cases-v1.json";
import type { LogEvent, Logger } from "../application/logging";
import type { CompositionPlan } from "../domain/project";
import {
  composition,
  interactiveComposition,
  threeSheetComposition,
} from "./albumCanvasTestFixtures";
import { createContinuousCanvasLayout } from "./canvasGeometry";
import {
  advancePixiTicker,
  AlbumCanvas,
  displayWithLabel,
  displayWithHandler,
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

test("aligns the focused Sheet outline with its exact geometry", async () => {
  renderCanvas();
  await finishPixiInitialization();

  expect(displayWithLabel("sheet-focus-sheet-001")).toMatchObject({
    rectCommands: [{ height: 300, width: 600, x: 0, y: 0 }],
    strokeStyles: [
      expect.objectContaining({
        alignment: 0.5,
        color: 0x2f7fba,
        pixelLine: true,
        width: 1,
      }),
    ],
  });
});

test("aligns the focused Sheet outline with the visible cut area in normal mode", async () => {
  renderCanvas({
    technicalGuides: { bleedUm: 3_000, safetyUm: 5_000 },
  });
  await finishPixiInitialization();

  expect(displayWithLabel("canvas-sheet-sheet-001")).toMatchObject({
    hitArea: { height: 294, width: 594, x: 3, y: 3 },
    position: { x: -3 },
  });
  expect(displayWithLabel("sheet-surface-sheet-001")).toMatchObject({
    rectCommands: [{ height: 294, width: 594, x: 3, y: 3 }],
  });
  expect(displayWithLabel("sheet-center-line-sheet-001")).toMatchObject({
    pathCommands: [
      { kind: "moveTo", x: 300, y: 3 },
      { kind: "lineTo", x: 300, y: 297 },
    ],
  });
  expect(displayWithLabel("sheet-bar-sheet-001")).toMatchObject({
    position: { x: 3, y: 3 },
  });
  expect(displayWithLabel("sheet-bar-surface-sheet-001")).toMatchObject({
    rectCommands: [{ height: 40, width: 594, x: 0, y: 0 }],
  });
  expect(displayWithLabel("sheet-bar-page-left-sheet-001")).toMatchObject({
    position: { x: 148.5 },
  });
  expect(displayWithLabel("sheet-bar-page-right-sheet-001")).toMatchObject({
    position: { x: 445.5 },
  });
  expect(displayWithLabel("sheet-focus-sheet-001")).toMatchObject({
    rectCommands: [{ height: 294, width: 594, x: 3, y: 3 }],
  });
});

test("matches the reference Canvas surface, cut-area crop and empty Frame treatment in normal mode", async () => {
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
    { height: 294, width: 594, x: 3, y: 3 },
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
  expect(placeholderBase.fillStyles).toContainEqual(
    expect.objectContaining({ color: 0xece8e1 }),
  );
  expect(displayWithLabel("frame-outline-frame-placeholder")).toMatchObject({
    strokeStyles: [
      expect.objectContaining({
        alpha: 0.88,
        color: 0xc9c2b7,
        pixelLine: true,
        width: 1,
      }),
    ],
  });
  const placeholderLabel = displayWithLabel(
    "frame-placeholder-label-frame-placeholder",
  );
  expect(placeholderLabel).toMatchObject({
    text: "Adicionar Foto",
    visible: true,
  });
  const expectedUiScale = 1 / ((500 - 2 * 28) / 300);
  expect(placeholderLabel.scale.x).toBeCloseTo(expectedUiScale);
  expect(placeholderLabel.scale.y).toBeCloseTo(expectedUiScale);
  expect(
    getPixiLifecycle().displays.some(
      ({ label }) => label === "frame-placeholder-stripes-frame-placeholder",
    ),
  ).toBe(false);

  const placeholderFrame = displayWithLabel(
    "canvas-frame-frame-placeholder",
  );
  expect(placeholderFrame.cursor).toBe("default");

  expect(
    pixiLifecycle.displays.some(({ label }) =>
      label.includes("sheet-bleed-guide-"),
    ),
  ).toBe(false);
  expect(
    pixiLifecycle.displays.some(({ label }) =>
      label.includes("sheet-safety-guide-"),
    ),
  ).toBe(false);
  const bleedMask = displayWithLabel("sheet-bleed-mask-sheet-001");
  expect(bleedMask).toMatchObject({
    rectCommands: [{ height: 294, width: 594, x: 3, y: 3 }],
  });
  expect(displayWithLabel("sheet-active-content-sheet-001").mask).toBe(
    bleedMask,
  );
});

test("shows the complete active surface and technical guides only in Sheet Edit Mode", async () => {
  renderCanvas({
    mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    technicalGuides: { bleedUm: 3_000, safetyUm: 5_000 },
  });
  await finishPixiInitialization();

  expect(displayWithLabel("sheet-bleed-guide-sheet-001")).toBeDefined();
  expect(displayWithLabel("sheet-safety-guide-sheet-001")).toBeDefined();
  expect(
    pixiLifecycle.displays.some(
      ({ label }) => label === "sheet-bleed-mask-sheet-001",
    ),
  ).toBe(false);
  expect(displayWithLabel("sheet-surface-sheet-001")).toMatchObject({
    rectCommands: [{ height: 300, width: 600, x: 0, y: 0 }],
  });
  expect(displayWithLabel("sheet-focus-sheet-001")).toMatchObject({
    rectCommands: [{ height: 300, width: 600, x: 0, y: 0 }],
  });
  expect(displayWithLabel("sheet-bar-sheet-001").visible).toBe(false);
});

test("shows the selected Frame boundary and eight visual-only resize handles in Sheet Edit Mode", async () => {
  renderCanvas({
    mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    compositionPlan: interactiveComposition,
    selectedFrameId: "frame-001",
    sheetBarMetadata: [
      {
        sheetId: "sheet-001",
        pageNumbers: [1, 2],
        layoutLocked: false,
      },
    ],
  });
  await finishPixiInitialization();

  expect(displayWithLabel("frame-selection-frame-001")).toMatchObject({
    rectCommands: [{ height: 200, width: 300, x: 0, y: 0 }],
    strokeStyles: [
      expect.objectContaining({
        alignment: 0.5,
        color: 0x2f7fba,
        pixelLine: true,
        width: 1,
      }),
    ],
    visible: true,
  });

  const handleLabels = [
    "top-left",
    "top",
    "top-right",
    "right",
    "bottom-right",
    "bottom",
    "bottom-left",
    "left",
  ].map(
    (position) =>
      `frame-resize-handle-placeholder-${position}-frame-001`,
  );
  const expectedPositions = [
    { x: 0, y: 0 },
    { x: 150, y: 0 },
    { x: 300, y: 0 },
    { x: 300, y: 100 },
    { x: 300, y: 200 },
    { x: 150, y: 200 },
    { x: 0, y: 200 },
    { x: 0, y: 100 },
  ];
  const expectedInverseScale = 1 / ((500 - 2 * 28) / 300);

  for (const [index, label] of handleLabels.entries()) {
    const handle = displayWithLabel(label) as unknown as {
      fillStyles: Array<{ color: number }>;
      position: { x: number; y: number };
      rectCommands: Array<{
        height: number;
        width: number;
        x: number;
        y: number;
      }>;
      scale: { x: number; y: number };
      strokeStyles: Array<{ color: number }>;
      visible: boolean;
    };
    expect(handle).toMatchObject({
      position: expectedPositions[index],
      rectCommands: [{ height: 8, width: 8, x: -4, y: -4 }],
      visible: true,
    });
    expect(handle.fillStyles).toContainEqual(
      expect.objectContaining({ color: 0xffffff }),
    );
    expect(handle.strokeStyles).toContainEqual(
      expect.objectContaining({ color: 0x2f7fba }),
    );
    expect(handle.scale.x).toBeCloseTo(expectedInverseScale);
    expect(handle.scale.y).toBeCloseTo(expectedInverseScale);
  }
});

test("keeps the selected Frame boundary but omits resize handles for a locked Layout", async () => {
  renderCanvas({
    mode: { kind: "sheet-editing", sheetId: "sheet-001" },
    compositionPlan: interactiveComposition,
    selectedFrameId: "frame-001",
    sheetBarMetadata: [
      {
        sheetId: "sheet-001",
        pageNumbers: [1, 2],
        layoutLocked: true,
      },
    ],
  });
  await finishPixiInitialization();

  expect(displayWithLabel("frame-selection-frame-001")).toMatchObject({
    visible: true,
  });
  expect(
    pixiLifecycle.displays.some(({ label }) =>
      label.startsWith("frame-resize-handle-placeholder-"),
    ),
  ).toBe(false);
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

test("does not drop on a new point while its resolved highlight is still pending", async () => {
  let resolveFirst!: (target: {
    kind: "frame";
    frameId: string;
  }) => void;
  let resolveSecond!: (target: {
    kind: "frame";
    frameId: string;
  }) => void;
  const onResolvePhotoDropTarget = vi
    .fn()
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveFirst = resolve;
        }),
    )
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveSecond = resolve;
        }),
    );
  const onDropPhoto = vi.fn(async () => true);
  const view = renderCanvas({
    compositionPlan: interactiveComposition,
    draggedPhotoId: "media-002",
    onResolvePhotoDropTarget,
    onDropPhoto,
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

  fireEvent.dragOver(host, { clientX: 560, clientY: 250, dataTransfer });
  await waitFor(() => expect(onResolvePhotoDropTarget).toHaveBeenCalledOnce());
  await act(async () => {
    resolveFirst({ kind: "frame", frameId: "frame-001" });
  });
  await waitFor(() => {
    expect(displayWithLabel("frame-photo-drop-frame-001").visible).toBe(true);
  });

  fireEvent.dragOver(host, { clientX: 680, clientY: 250, dataTransfer });
  await waitFor(() => expect(onResolvePhotoDropTarget).toHaveBeenCalledTimes(2));
  fireEvent.drop(host, { clientX: 680, clientY: 250, dataTransfer });

  expect(onDropPhoto).not.toHaveBeenCalled();
  await act(async () => {
    resolveSecond({ kind: "frame", frameId: "frame-001" });
  });
  expect(displayWithLabel("frame-photo-drop-frame-001").visible).toBe(false);
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
  const onEditSheet = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onEditSheet,
  });
  await finishPixiInitialization();
  const sheet = displayWithHandler("pointertap");

  sheet.emit("pointertap", { button: 0, target: sheet, detail: 2 });
  expect(onEditSheet).toHaveBeenCalledWith("sheet-001");

  displayWithLabel("canvas-frame-frame-001").emit("pointertap", {
    button: 0,
    detail: 2,
    stopPropagation: vi.fn(),
  });
  expect(onEditSheet).toHaveBeenCalledTimes(2);
  expect(onEditSheet).toHaveBeenLastCalledWith("sheet-001");
});

test("leaves right-button taps on the Pixi surface, Frame, and Sheet Bar to the contextual owner", async () => {
  const onEditSheet = vi.fn();
  const onFocusSheet = vi.fn();
  const onSelectFrame = vi.fn();
  renderCanvas({
    compositionPlan: interactiveComposition,
    onEditSheet,
    onFocusSheet,
    onSelectFrame,
  });
  await finishPixiInitialization();

  const sheet = displayWithLabel("canvas-sheet-sheet-001");
  const frame = displayWithLabel("canvas-frame-frame-001");
  const sheetBar = displayWithLabel("sheet-bar-sheet-001");
  sheet.emit("pointertap", {
    button: 2,
    detail: 1,
    target: sheet,
  });
  frame.emit("pointertap", {
    altKey: false,
    button: 2,
    detail: 1,
    stopPropagation: vi.fn(),
  });
  sheetBar.emit("pointertap", {
    button: 2,
    detail: 1,
    target: sheetBar,
  });

  expect(onFocusSheet).not.toHaveBeenCalled();
  expect(onSelectFrame).not.toHaveBeenCalled();
  expect(onEditSheet).not.toHaveBeenCalled();
});

test("editing mode materializes only its isolated sheet", async () => {
  renderCanvas({
    compositionPlan: threeSheetComposition,
    mode: { kind: "sheet-editing", sheetId: "sheet-002" },
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

test.each([
  {
    guides: { bleedUm: 0, safetyUm: 5_000 },
    expectedLabels: [],
  },
  {
    guides: { bleedUm: 3_000, safetyUm: 0 },
    expectedLabels: ["sheet-bleed-mask-sheet-001"],
  },
  { guides: { bleedUm: 0, safetyUm: 0 }, expectedLabels: [] },
])(
  "keeps technical guides hidden and applies only the normal-mode crop ($guides)",
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
    guides: { bleedUm: 0, safetyUm: 5_000 },
    expectedLabels: ["sheet-safety-guide-sheet-001"],
  },
  {
    guides: { bleedUm: 3_000, safetyUm: 0 },
    expectedLabels: ["sheet-bleed-guide-sheet-001"],
  },
  { guides: { bleedUm: 0, safetyUm: 0 }, expectedLabels: [] },
])(
  "disables zero-valued guides independently in Sheet Edit Mode ($guides)",
  async ({ guides, expectedLabels }) => {
    renderCanvas({
      mode: { kind: "sheet-editing", sheetId: "sheet-001" },
      technicalGuides: guides,
    });
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

const singlePageEdgeCases = [
  {
    activeSides: "right" as const,
    activeMask: { height: 294, width: 297, x: 300, y: 3 },
    omittedGuideX: 3,
    inactiveBoundaryX: 0,
    inactivePosition: { x: 3, y: 3 },
    retainedGuideX: 297,
  },
  {
    activeSides: "left" as const,
    activeMask: { height: 294, width: 297, x: 3, y: 3 },
    omittedGuideX: 297,
    inactiveBoundaryX: 300,
    inactivePosition: { x: 300, y: 3 },
    retainedGuideX: 3,
  },
] as const;

test.each(singlePageEdgeCases)(
  "omits technical guides on the inactive-side edge of a $activeSides single Page in Sheet Edit Mode",
  async ({
    activeSides,
    inactiveBoundaryX,
    omittedGuideX,
    retainedGuideX,
  }) => {
    renderCanvas({
      compositionPlan: createSinglePageComposition(activeSides),
      mode: { kind: "sheet-editing", sheetId: "sheet-001" },
      sheetBarMetadata: [
        {
          sheetId: "sheet-001",
          pageNumbers: [1],
          layoutLocked: false,
        },
      ],
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

    expect(
      pixiLifecycle.displays.some(
        ({ label }) => label === "sheet-bleed-mask-sheet-001",
      ),
    ).toBe(false);
  },
);

test.each(singlePageEdgeCases)(
  "clips the active $activeSides single Page and keeps the inactive side inside the same visible outline",
  async ({ activeSides, activeMask, inactivePosition }) => {
    renderCanvas({
      compositionPlan: createSinglePageComposition(activeSides),
      sheetBarMetadata: [
        {
          sheetId: "sheet-001",
          pageNumbers: [1],
          layoutLocked: false,
        },
      ],
      technicalGuides: { bleedUm: 3_000, safetyUm: 5_000 },
    });
    await finishPixiInitialization();

    const bleedMask = displayWithLabel("sheet-bleed-mask-sheet-001");
    expect(bleedMask).toMatchObject({ rectCommands: [activeMask] });
    expect(displayWithLabel("sheet-active-content-sheet-001").mask).toBe(
      bleedMask,
    );
    expect(displayWithLabel("canvas-sheet-sheet-001")).toMatchObject({
      hitArea: { height: 294, width: 594, x: 3, y: 3 },
    });
    expect(displayWithLabel("sheet-focus-sheet-001")).toMatchObject({
      rectCommands: [{ height: 294, width: 594, x: 3, y: 3 }],
    });
    expect(displayWithLabel("sheet-inactive-side-sheet-001")).toMatchObject({
      position: inactivePosition,
    });
    expect(
      displayWithLabel("sheet-inactive-side-gradient-sheet-001"),
    ).toMatchObject({
      rectCommands: [{ height: 294, width: 297, x: 0, y: 0 }],
    });
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
    alpha: 0.8,
    eventMode: "static",
    tint: 0x403b35,
  });
  expect(layoutAction).toMatchObject({
    alpha: 0.8,
    eventMode: "static",
    tint: 0x403b35,
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
    expect(action).toMatchObject({ alpha: 0.8, tint: 0x403b35 });
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

test("enters Sheet Edit Mode on the second pointer tap of a Sheet", async () => {
  const onEditSheet = vi.fn();
  renderCanvas({ onEditSheet });
  await finishPixiInitialization();

  const sheet = displayWithLabel("canvas-sheet-sheet-001");
  sheet.emit("pointertap", { button: 0, detail: 1, target: sheet });
  expect(onEditSheet).not.toHaveBeenCalled();

  sheet.emit("pointertap", { button: 0, detail: 2, target: sheet });
  expect(onEditSheet).toHaveBeenCalledWith("sheet-001");
});

test.each([
  {
    activeSides: "right" as const,
    activeOffsetXPx: 300,
    gradientEndX: 1,
    gradientStartX: 0,
    inactiveOffsetXPx: 0,
    pageX: 450,
    missingPageLabel: "sheet-bar-page-left-sheet-001",
  },
  {
    activeSides: "left" as const,
    activeOffsetXPx: 0,
    gradientEndX: 0,
    gradientStartX: 1,
    inactiveOffsetXPx: 300,
    pageX: 150,
    missingPageLabel: "sheet-bar-page-right-sheet-001",
  },
])(
  "presents the inactive $activeSides edge side without making it content",
  async ({
    activeSides,
    activeOffsetXPx,
    gradientEndX,
    gradientStartX,
    inactiveOffsetXPx,
    missingPageLabel,
    pageX,
  }) => {
    const onFocusSheet = vi.fn();
    const view = renderCanvas({
      compositionPlan: createSinglePageComposition(activeSides),
      sheetBarMetadata: [
        {
          sheetId: "sheet-001",
          pageNumbers: [1],
          layoutLocked: false,
        },
      ],
      onFocusSheet,
    });
    await finishPixiInitialization();

    expect(displayWithLabel("sheet-surface-sheet-001")).toMatchObject({
      rectCommands: [{ height: 300, width: 600, x: 0, y: 0 }],
    });
    expect(displayWithLabel("sheet-inactive-side-sheet-001")).toMatchObject({
      eventMode: "none",
      position: { x: inactiveOffsetXPx, y: 0 },
    });
    expect(displayWithLabel("sheet-inactive-side-gradient-sheet-001")).toMatchObject({
      fillStyles: [
        expect.objectContaining({
          colorStops: [
            { color: "#faf9f6", offset: 0 },
            { color: "#ebe3d8", offset: 0.58 },
            { color: "#cec2b2", offset: 1 },
          ],
          end: { x: gradientEndX, y: 0 },
          start: { x: gradientStartX, y: 0 },
          type: "linear",
        }),
      ],
      rectCommands: [{ height: 300, width: 300, x: 0, y: 0 }],
    });
    expect(
      pixiLifecycle.displays.some(
        ({ label }) =>
          label === "sheet-inactive-side-fold-shadow-sheet-001" ||
          label === "sheet-inactive-side-stripes-sheet-001",
      ),
    ).toBe(false);
    expect(displayWithLabel("sheet-active-content-sheet-001")).toMatchObject({
      position: { x: activeOffsetXPx, y: 0 },
    });
    expect(displayWithLabel("canvas-sheet-sheet-001")).toMatchObject({
      hitArea: { height: 300, width: 600, x: 0, y: 0 },
    });
    const sheetBar = displayWithLabel("sheet-bar-sheet-001");
    expect(sheetBar).toMatchObject({
      hitArea: { height: 40, width: 600, x: 0, y: 0 },
    });
    sheetBar.emit("pointertap", { button: 0, target: sheetBar });
    expect(onFocusSheet).toHaveBeenCalledWith("sheet-001");
    expect(displayWithLabel("sheet-center-line-sheet-001")).toMatchObject({
      pathCommands: [
        { kind: "moveTo", x: 300, y: 0 },
        { kind: "lineTo", x: 300, y: 300 },
      ],
    });
    expect(displayWithLabel("sheet-focus-sheet-001")).toMatchObject({
      rectCommands: [{ height: 300, width: 600, x: 0, y: 0 }],
    });
    expect(displayWithLabel(`sheet-bar-page-${activeSides}-sheet-001`)).toMatchObject({
      position: { x: pageX, y: 20 },
      text: "1",
    });
    expect(
      pixiLifecycle.displays.some(({ label }) => label === missingPageLabel),
    ).toBe(false);
    const gradient =
      pixiLifecycle.fillGradients[pixiLifecycle.fillGradients.length - 1];
    expect(gradient?.destroyCount).toBe(0);
    view.unmount();
    expect(gradient?.destroyCount).toBe(1);
  },
);

test("consumes the registered final-renderer case for gapless active Page labels", async () => {
  const adapted = adaptFinalRendererCaseForCanvas("single-active-edge-pages");

  expect(adapted.adapterRegistrations).toEqual([
    "composition-core",
    "canvas",
    "export-pipeline",
  ]);
  expect(adapted.caseId).toBe("single-active-edge-pages");
  renderCanvas({
    compositionPlan: adapted.compositionPlan,
    sheetBarMetadata: adapted.sheetBarMetadata,
  });
  await finishPixiInitialization();

  expect(displayWithLabel("sheet-bar-page-right-edge-initial")).toMatchObject({
    text: "1",
  });
  expect(displayWithLabel("sheet-bar-page-left-edge-internal")).toMatchObject({
    text: "2",
  });
  expect(displayWithLabel("sheet-bar-page-right-edge-internal")).toMatchObject({
    text: "3",
  });
  expect(displayWithLabel("sheet-bar-page-left-edge-final")).toMatchObject({
    text: "4",
  });
  expect(
    pixiLifecycle.displays.some(
      ({ label }) => label === "sheet-bar-page-left-edge-initial",
    ),
  ).toBe(false);
  expect(
    pixiLifecycle.displays.some(
      ({ label }) => label === "sheet-bar-page-right-edge-final",
    ),
  ).toBe(false);

  expectCanvasPosition("sheet-active-content-edge-initial", 25.4, 0);
  expectCanvasPosition("sheet-active-content-edge-internal", 0, 0);
  expectCanvasPosition("sheet-active-content-edge-final", 0, 0);
  expect(
    backgroundLabels("sheet-active-content-edge-initial"),
  ).toEqual(["background-color-#0000FF"]);
  expect(
    backgroundLabels("sheet-active-content-edge-internal"),
  ).toEqual(["background-color-#00FF00", "background-color-#FF00FF"]);
  expect(backgroundLabels("sheet-active-content-edge-final")).toEqual([
    "background-color-#FFFF00",
  ]);
  expectCanvasRect("background-color-#0000FF", [0, 0, 25.4, 25.4]);
  expectCanvasRect("background-color-#00FF00", [0, 0, 25.4, 25.4]);
  expectCanvasRect("background-color-#FF00FF", [25.4, 0, 25.4, 25.4]);
  expectCanvasRect("background-color-#FFFF00", [0, 0, 25.4, 25.4]);
  expect(
    pixiLifecycle.displays.some(
      ({ label }) =>
        label === "background-color-#FF0000" ||
        label === "background-color-#00FFFF",
    ),
  ).toBe(false);
});

function expectCanvasPosition(label: string, x: number, y: number): void {
  const display = displayWithLabel(label) as unknown as {
    position: { x: number; y: number };
  };
  expect(display.position.x).toBeCloseTo(x, 8);
  expect(display.position.y).toBeCloseTo(y, 8);
}

function expectCanvasRect(
  label: string,
  [x, y, width, height]: GoldenRectV1,
): void {
  const display = displayWithLabel(label) as unknown as {
    rectCommands: Array<{
      x: number;
      y: number;
      width: number;
      height: number;
    }>;
  };
  expect(display.rectCommands).toHaveLength(1);
  expect(display.rectCommands[0].x).toBeCloseTo(x, 8);
  expect(display.rectCommands[0].y).toBeCloseTo(y, 8);
  expect(display.rectCommands[0].width).toBeCloseTo(width, 8);
  expect(display.rectCommands[0].height).toBeCloseTo(height, 8);
}

function backgroundLabels(activeContentLabel: string): string[] {
  const activeContent = displayWithLabel(activeContentLabel) as unknown as {
    children: Array<{ label?: string }>;
  };
  return activeContent.children
    .map(({ label }) => label)
    .filter(
      (label): label is string => label?.startsWith("background-color-") === true,
    );
}

type GoldenRectV1 = [number, number, number, number];

type GoldenCanvasLayerV1 =
  | {
      kind: "base";
      data: { rectUm: GoldenRectV1; rgb: string };
    }
  | {
      kind: "background";
      data: {
        rectUm: GoldenRectV1;
        paint: { kind: "solid"; data: { rgb: string } };
      };
    };

interface GoldenCanvasCompositionCase {
  id: string;
  adapterRegistrations: string[];
  input: {
    creativeState: {
      sheets: Array<{
        sheetId: string;
        number: number;
        activeSides: "both" | "left" | "right";
      }>;
    };
  };
  expectedPlan: {
    sheets: Array<{
      sheetId: string;
      surfaceRectUm: GoldenRectV1;
      orderedLayers: GoldenCanvasLayerV1[];
      outputUnits: Array<{
        mode: "per-page" | "per-sheet";
        logicalIndex: number;
      }>;
    }>;
  };
}

function adaptFinalRendererCaseForCanvas(caseId: string) {
  const rawCase = (finalRendererCorpus.cases as readonly unknown[]).find(
    (candidate): candidate is {
      kind: "composition";
      data: GoldenCanvasCompositionCase;
    } => {
      if (typeof candidate !== "object" || candidate === null) {
        return false;
      }
      const record = candidate as { kind?: unknown; data?: { id?: unknown } };
      return record.kind === "composition" && record.data?.id === caseId;
    },
  );
  if (!rawCase) {
    throw new Error(`final-renderer Canvas case not found: ${caseId}`);
  }
  if (!rawCase.data.adapterRegistrations.includes("canvas")) {
    throw new Error(`final-renderer case is not registered for Canvas: ${caseId}`);
  }

  const creativeSheets = new Map(
    rawCase.data.input.creativeState.sheets.map((sheet) => [
      sheet.sheetId,
      sheet,
    ]),
  );
  const compositionPlan: CompositionPlan = {
    frameBorder: { kind: "none" },
    sheets: rawCase.data.expectedPlan.sheets.map((expectedSheet) => {
      const creativeSheet = creativeSheets.get(expectedSheet.sheetId);
      const [base, ...contentLayers] = expectedSheet.orderedLayers;
      if (!creativeSheet || base?.kind !== "base") {
        throw new Error(`missing creative Sheet: ${expectedSheet.sheetId}`);
      }
      const [, , widthUm, heightUm] = expectedSheet.surfaceRectUm;
      const backgrounds = contentLayers.map((layer) => {
        if (
          layer.kind !== "background" ||
          layer.data.paint.kind !== "solid"
        ) {
          throw new Error(
            `unsupported Canvas layer in final-renderer case: ${expectedSheet.sheetId}`,
          );
        }
        const [x, y, width, height] = layer.data.rectUm;
        return {
          kind: "color" as const,
          rgb: layer.data.paint.data.rgb,
          drawRect: { x, y, width, height },
        };
      });
      const [baseX, baseY, baseWidth, baseHeight] = base.data.rectUm;
      return {
        sheetId: expectedSheet.sheetId,
        number: creativeSheet.number,
        activeSides: creativeSheet.activeSides,
        widthUm,
        heightUm,
        base: {
          rgb: base.data.rgb,
          drawRect: {
            x: baseX,
            y: baseY,
            width: baseWidth,
            height: baseHeight,
          },
        },
        backgrounds,
        overlays: [],
        frames: [],
      };
    }),
  };
  const sheetBarMetadata = rawCase.data.expectedPlan.sheets.map((sheet) => ({
    sheetId: sheet.sheetId,
    pageNumbers: sheet.outputUnits
      .filter((unit) => unit.mode === "per-page")
      .map((unit) => unit.logicalIndex),
    layoutLocked: false,
  }));

  return {
    caseId: rawCase.data.id,
    adapterRegistrations: rawCase.data.adapterRegistrations,
    compositionPlan,
    sheetBarMetadata,
  };
}

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
    onEditSheet: vi.fn(),
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
      mode={{ kind: "normal" }}
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
    onEditSheet: vi.fn(),
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
      mode={{ kind: "normal" }}
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
    mode: { kind: "normal" } as const,
    sheetBarMetadata: [],
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 42 },
    onSelectFrame: vi.fn(),
    onEditSheet: vi.fn(),
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

test("opens the structural context menu for the Sheet under the pointer", async () => {
  const onOpenSheetContextMenu = vi.fn();
  const view = renderCanvas({ onOpenSheetContextMenu });
  await finishPixiInitialization();

  const host = view.container.querySelector(".canvas-host") as HTMLDivElement;
  const canvas = view.container.querySelector("canvas") as HTMLCanvasElement;
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
    bottom: 500,
    height: 500,
    left: 0,
    right: 1_000,
    top: 0,
    width: 1_000,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });

  fireEvent.contextMenu(host, { clientX: 600, clientY: 250 });

  expect(onOpenSheetContextMenu).toHaveBeenCalledWith("sheet-001", {
    x: 600,
    y: 250,
  });
});

test("opens the structural context menu from the inactive side of a single Page Sheet", async () => {
  const onOpenSheetContextMenu = vi.fn();
  const view = renderCanvas({
    compositionPlan: createSinglePageComposition("right"),
    onOpenSheetContextMenu,
    technicalGuides: { bleedUm: 3_000, safetyUm: 3_000 },
  });
  await finishPixiInitialization();

  const host = view.container.querySelector(".canvas-host") as HTMLDivElement;
  const canvas = view.container.querySelector("canvas") as HTMLCanvasElement;
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
    bottom: 500,
    height: 500,
    left: 0,
    right: 1_000,
    top: 0,
    width: 1_000,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });

  fireEvent.contextMenu(host, { clientX: 220, clientY: 250 });

  expect(onOpenSheetContextMenu).toHaveBeenCalledWith("sheet-001", {
    x: 220,
    y: 250,
  });
});

test("does not expose the structural context menu through masked Sangria", async () => {
  const onOpenSheetContextMenu = vi.fn();
  const view = renderCanvas({
    onOpenSheetContextMenu,
    technicalGuides: { bleedUm: 3_000, safetyUm: 3_000 },
  });
  await finishPixiInitialization();

  const host = view.container.querySelector(".canvas-host") as HTMLDivElement;
  const canvas = view.container.querySelector("canvas") as HTMLCanvasElement;
  vi.spyOn(canvas, "getBoundingClientRect").mockReturnValue({
    bottom: 500,
    height: 500,
    left: 0,
    right: 1_000,
    top: 0,
    width: 1_000,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });

  fireEvent.contextMenu(host, { clientX: 132, clientY: 250 });

  expect(onOpenSheetContextMenu).not.toHaveBeenCalled();
});

test("mounts the productive Sheet Bar seam for navigation, edit, context, and reorder", async () => {
  const onCancel = vi.fn();
  const onDrop = vi.fn();
  const onNavigate = vi.fn();
  const onOpenSheetContextMenu = vi.fn();
  const onPreview = vi.fn();
  const view = renderCanvas({
    compositionPlan: threeSheetComposition,
    onOpenSheetContextMenu,
    sheetReorder: {
      disabled: false,
      onCancel,
      onDrop,
      onNavigate,
      onPreview,
      representation: {
        ghost: null,
        order: ["sheet-001", "sheet-002", "sheet-003"],
        placeholderIndex: null,
      },
      status: "idle",
    },
  });
  await finishPixiInitialization();

  const first = screen.getByRole("button", {
    name: "Reordenar Lâmina 01 pela Barra",
  });
  const second = screen.getByRole("button", {
    name: "Reordenar Lâmina 02 pela Barra",
  });
  const overlay = view.getByRole("group", {
    name: "Reordenação pela Barra da Lâmina",
  });
  vi.spyOn(overlay, "getBoundingClientRect").mockReturnValue({
    left: 0,
    right: 10_000,
    top: 0,
    bottom: 600,
    width: 10_000,
    height: 600,
    x: 0,
    y: 0,
    toJSON: () => ({}),
  });

  fireEvent.click(second);
  expect(onNavigate).toHaveBeenCalledWith("sheet-002");
  const firstWidth = Number.parseFloat(first.style.width);
  const secondWidth = Number.parseFloat(second.style.width);
  fireEvent.doubleClick(first, {
    clientX: Number.parseFloat(first.style.left) + firstWidth * 0.25,
    clientY: 36,
  });
  fireEvent.doubleClick(second, {
    clientX: Number.parseFloat(second.style.left) + secondWidth * 0.75,
    clientY: 36,
  });
  expect(view.onEditSheet).toHaveBeenNthCalledWith(1, "sheet-001");
  expect(view.onEditSheet).toHaveBeenNthCalledWith(2, "sheet-002");
  fireEvent.contextMenu(second, { clientX: 220, clientY: 36 });
  expect(onOpenSheetContextMenu).toHaveBeenCalledWith("sheet-002", {
    x: 220,
    y: 36,
  });

  const firstX = Number.parseFloat(first.style.left) + 8;
  const secondX = Number.parseFloat(second.style.left) + 8;
  fireEvent.pointerDown(first, {
    button: 0,
    buttons: 1,
    clientX: firstX,
    clientY: 32,
    pointerId: 31,
    pointerType: "mouse",
  });
  fireEvent.pointerMove(first, {
    buttons: 1,
    clientX: secondX,
    clientY: 32,
    pointerId: 31,
    pointerType: "mouse",
  });
  expect(onPreview).toHaveBeenLastCalledWith("sheet-001", 1);
  fireEvent.pointerUp(first, {
    button: 0,
    buttons: 0,
    clientX: secondX,
    clientY: 32,
    pointerId: 31,
    pointerType: "mouse",
  });
  expect(onDrop).toHaveBeenCalledOnce();
});

test("slides intermediate Pixi Sheets while the dragged Sheet yields to the placeholder", async () => {
  const canvasProps = {
    projectId: "project-spike-001",
    mode: { kind: "normal" } as const,
    composition: threeSheetComposition,
    continuousCanvasLayout: createContinuousCanvasLayout(
      threeSheetComposition.sheets,
    ),
    sheetBarMetadata: [],
    selectedFrameId: null,
    focusedSheetId: "sheet-001",
    centeredSheetId: "sheet-001",
    viewport: { offsetX: 42 },
    onSelectFrame: vi.fn(),
    onEditSheet: vi.fn(),
    onFocusSheet: vi.fn(),
    onCenteredSheetChange: vi.fn(),
    onViewportChange: vi.fn(),
    onTransformPreview: vi.fn(),
    onTransformCommit: vi.fn(async () => true),
  };
  const reorderCallbacks = {
    disabled: false,
    onCancel: vi.fn(),
    onDrop: vi.fn(),
    onNavigate: vi.fn(),
    onPreview: vi.fn(),
  };
  const view = render(
    <AlbumCanvas
      {...canvasProps}
      sheetReorder={{
        ...reorderCallbacks,
        representation: {
          ghost: null,
          order: ["sheet-001", "sheet-002", "sheet-003"],
          placeholderIndex: null,
        },
        status: "idle",
      }}
    />,
  );
  await finishPixiInitialization();

  const first = displayWithLabel("canvas-sheet-sheet-001");
  const second = displayWithLabel("canvas-sheet-sheet-002");
  const third = displayWithLabel("canvas-sheet-sheet-003");
  const initialPositions = [
    first.position.x,
    second.position.x,
    third.position.x,
  ];

  view.rerender(
    <AlbumCanvas
      {...canvasProps}
      sheetReorder={{
        ...reorderCallbacks,
        representation: {
          ghost: { sheetId: "sheet-003" },
          order: ["sheet-001", "sheet-003", "sheet-002"],
          placeholderIndex: 1,
        },
        status: "preview",
      }}
    />,
  );

  expect(first.position.x).toBe(initialPositions[0]);
  expect(second.position.x).toBe(initialPositions[1]);
  expect(third.visible).toBe(false);

  await advancePixiTicker(70);
  expect(second.position.x).toBeGreaterThan(initialPositions[1]);
  expect(second.position.x).toBeLessThan(initialPositions[2]);

  await advancePixiTicker(70);
  expect(second.position.x).toBe(initialPositions[2]);

  view.rerender(
    <AlbumCanvas
      {...canvasProps}
      sheetReorder={{
        ...reorderCallbacks,
        representation: {
          ghost: { sheetId: "sheet-003" },
          order: ["sheet-001", "sheet-003", "sheet-002"],
          placeholderIndex: 1,
        },
        status: "committing",
      }}
    />,
  );
  await advancePixiTicker(70);
  expect(second.position.x).toBe(initialPositions[2]);
  expect(third.visible).toBe(false);

  view.rerender(
    <AlbumCanvas
      {...canvasProps}
      sheetReorder={{
        ...reorderCallbacks,
        representation: {
          ghost: null,
          order: ["sheet-001", "sheet-002", "sheet-003"],
          placeholderIndex: null,
        },
        status: "cancelled",
      }}
    />,
  );
  expect(third.visible).toBe(true);
  await advancePixiTicker(140);
  expect(second.position.x).toBe(initialPositions[1]);
});
