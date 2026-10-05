import { expect, test } from "vitest";
import {
  finishPixiInitialization,
  getPixiLifecycle,
  renderCanvas,
  setupAlbumCanvasTestHarness,
} from "./albumCanvasTestHarness";

import type { ComposedPhoto, CompositionPlan } from "../../domain/project";
import { interactiveComposition } from "./albumCanvasTestFixtures";

setupAlbumCanvasTestHarness();
const pixiLifecycle = getPixiLifecycle();

const PHOTO_LAYER_LABELS = [
  "photo-pan-inside-preview",
  "photo-pan-outside-preview",
] as const;

function compositionWithPhoto(
  overrides: Partial<ComposedPhoto>,
): CompositionPlan {
  const sheet = interactiveComposition.sheets[0];
  return {
    ...interactiveComposition,
    sheets: [
      {
        ...sheet,
        frames: [
          {
            ...sheet.frames[0],
            photo: { ...sheet.frames[0].photo!, ...overrides },
          },
        ],
      },
    ],
  };
}

function latestPhotoLayer(label: (typeof PHOTO_LAYER_LABELS)[number]) {
  const layers = pixiLifecycle.displays.filter(
    (display) => display.label === label,
  );
  const layer = layers[layers.length - 1];
  if (!layer) throw new Error(`Pixi display labeled ${label} not found`);
  return layer as typeof layer & { rotation: number };
}

test("a colour Photo carries no filter and builds no black-and-white program", async () => {
  renderCanvas({
    compositionPlan: compositionWithPhoto({ blackAndWhite: false }),
  });
  await finishPixiInitialization();

  for (const label of PHOTO_LAYER_LABELS) {
    expect(latestPhotoLayer(label).filters).toEqual([]);
  }
  expect(pixiLifecycle.filters).toEqual([]);
});

test("a black-and-white Photo filters each of its layers with its own integer-luminance program", async () => {
  renderCanvas({
    compositionPlan: compositionWithPhoto({ blackAndWhite: true }),
  });
  await finishPixiInitialization();

  // The clipped Photo and the dimmed Pan overflow are separate layers.
  expect(pixiLifecycle.filters).toHaveLength(2);
  const attached = PHOTO_LAYER_LABELS.map((label) => {
    const { filters } = latestPhotoLayer(label);
    expect(filters).toHaveLength(1);
    return filters[0];
  });
  expect(attached[0]).not.toBe(attached[1]);
  expect(new Set(attached)).toEqual(new Set(pixiLifecycle.filters));
  for (const filter of pixiLifecycle.filters) {
    expect(filter.options.glProgram?.options).toMatchObject({
      name: "photo-black-and-white",
      fragment: expect.stringContaining("vec3(54.0, 183.0, 19.0)"),
    });
    expect(filter.destroyCalls).toEqual([]);
  }
});

test("switching black and white rebuilds the Photo layers and releases each retired filter with its program", async () => {
  const view = renderCanvas({
    compositionPlan: compositionWithPhoto({ blackAndWhite: true }),
  });
  await finishPixiInitialization();
  const first = [...pixiLifecycle.filters];
  expect(first).toHaveLength(2);

  view.rerenderCanvas({
    composition: compositionWithPhoto({ blackAndWhite: false }),
  });

  for (const label of PHOTO_LAYER_LABELS) {
    expect(latestPhotoLayer(label).filters).toEqual([]);
  }
  expect(pixiLifecycle.filters).toHaveLength(2);
  for (const filter of first) expect(filter.destroyCalls).toEqual([[true]]);

  view.rerenderCanvas({
    composition: compositionWithPhoto({ blackAndWhite: true }),
  });

  const second = pixiLifecycle.filters.slice(2);
  expect(second).toHaveLength(2);
  for (const label of PHOTO_LAYER_LABELS) {
    expect(second).toContain(latestPhotoLayer(label).filters[0]);
  }
  for (const filter of second) expect(filter.destroyCalls).toEqual([]);

  view.unmount();

  for (const filter of [...first, ...second]) {
    expect(filter.destroyCalls).toEqual([[true]]);
  }
});

// Design 0021: Mirroring flips the horizontal of the already rotated Photo.
// Pixi applies scale before rotation, so a local vector v is drawn at
// R(rotation) * S(scale) * v, while the design asks for F * R(angle) * v with
// F flipping X only when mirrored.
test.each([
  { rotationDegrees: 0, mirrorX: false, scaleX: 1, rotation: 0 },
  { rotationDegrees: 0, mirrorX: true, scaleX: -1, rotation: 0 },
  { rotationDegrees: 90, mirrorX: false, scaleX: 1, rotation: Math.PI / 2 },
  { rotationDegrees: 90, mirrorX: true, scaleX: -1, rotation: -Math.PI / 2 },
  { rotationDegrees: 30, mirrorX: false, scaleX: 1, rotation: Math.PI / 6 },
  { rotationDegrees: 30, mirrorX: true, scaleX: -1, rotation: -Math.PI / 6 },
])(
  "draws a Photo rotated $rotationDegrees degrees with mirror $mirrorX as rotate-then-mirror",
  async ({ rotationDegrees, mirrorX, scaleX, rotation }) => {
    renderCanvas({
      compositionPlan: compositionWithPhoto({ rotationDegrees, mirrorX }),
    });
    await finishPixiInitialization();

    const angle = (rotationDegrees * Math.PI) / 180;
    for (const label of PHOTO_LAYER_LABELS) {
      const layer = latestPhotoLayer(label);
      expect(layer.scale).toMatchObject({ x: scaleX, y: 1 });
      expect(layer.rotation).toBeCloseTo(rotation, 12);

      for (const [x, y] of [
        [1, 0],
        [0, 1],
      ]) {
        const scaledX = x * layer.scale.x;
        const scaledY = y * layer.scale.y;
        const drawnX =
          Math.cos(layer.rotation) * scaledX -
          Math.sin(layer.rotation) * scaledY;
        const drawnY =
          Math.sin(layer.rotation) * scaledX +
          Math.cos(layer.rotation) * scaledY;
        const rotatedX = Math.cos(angle) * x - Math.sin(angle) * y;
        const rotatedY = Math.sin(angle) * x + Math.cos(angle) * y;

        expect(drawnX).toBeCloseTo(mirrorX ? -rotatedX : rotatedX, 12);
        expect(drawnY).toBeCloseTo(rotatedY, 12);
      }
    }
  },
);
