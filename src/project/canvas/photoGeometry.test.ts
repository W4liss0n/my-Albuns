// @vitest-environment node
import { expect, test } from "vitest";

import placementFixture from "../../../tests/fixtures/photo-placement-cases.json";
import type {
  NormalizedPan,
  PhotoPlacement,
  PhotoPlacementPlan,
  VectorUm,
} from "../../domain/project";
import {
  createPhotoGeometry,
  type CanvasPhotoPlacement,
} from "./photoGeometry";

interface PanPreview {
  center: VectorUm;
  expectedPan: NormalizedPan;
  expectedPlacement: PhotoPlacement;
}

interface ZoomPreview {
  zoom: number;
  expectedPan: NormalizedPan;
  expectedPlacement: PhotoPlacement;
}

interface PlacementCase {
  name: string;
  expectedPlan: PhotoPlacementPlan;
  panPreviews: PanPreview[];
  zoomPreviews: ZoomPreview[];
}

const placementCases = placementFixture.cases as PlacementCase[];

test("evaluates the shared CompositionCore placement contract", () => {
  for (const placementCase of placementCases) {
    const geometry = createPhotoGeometry(placementCase.expectedPlan);

    expectPlacementClose(
      geometry.current,
      placementCase.expectedPlan.current,
      placementCase.name,
    );

    for (const preview of placementCase.zoomPreviews) {
      const zoomed = geometry.zoom(preview.zoom);
      expect(zoomed.zoom).toBeCloseTo(preview.zoom, 9);
      expect(zoomed.pan.x).toBeCloseTo(preview.expectedPan.x, 6);
      expect(zoomed.pan.y).toBeCloseTo(preview.expectedPan.y, 6);
      expectPlacementClose(
        zoomed.placement,
        preview.expectedPlacement,
        placementCase.name,
      );
    }

    for (const preview of placementCase.panPreviews) {
      const constrained = geometry.constrain(preview.center);
      expect(constrained.pan.x).toBeCloseTo(
        preview.expectedPan.x,
        6,
      );
      expect(constrained.pan.y).toBeCloseTo(
        preview.expectedPan.y,
        6,
      );
      expectPlacementClose(
        constrained.placement,
        preview.expectedPlacement,
        placementCase.name,
      );
    }
  }
});

test("adapts the plan units once at the renderer seam", () => {
  const geometry = createPhotoGeometry(
    placementCases[0].expectedPlan,
    0.001,
  );

  expect(geometry.current.center.x).toBeCloseTo(105, 6);
  expect(geometry.current.center.y).toBeCloseTo(100, 6);
  expect(geometry.current.size.width).toBeCloseTo(400, 6);
  expect(geometry.current.size.height).toBeCloseTo(200, 6);
});

test("constrains Pan against transient Zoom without moving the preview", () => {
  const geometry = createPhotoGeometry(
    placementCases[0].expectedPlan,
  );

  const combined = geometry.constrain(
    geometry.current.center,
    1.12,
  );

  expect(combined.zoom).toBeCloseTo(1.12, 6);
  expect(combined.pan.x).toBeCloseTo(-45_000 / 74_000, 6);
  expect(combined.placement.center.x).toBeCloseTo(105_000, 6);
  expect(combined.placement.center.y).toBeCloseTo(100_000, 6);
  expect(combined.placement.size.width).toBeCloseTo(448_000, 6);
  expect(combined.placement.size.height).toBeCloseTo(224_000, 6);

  const rotatedGeometry = createPhotoGeometry(
    placementCases[1].expectedPlan,
  );
  const rotated = rotatedGeometry.constrain(
    rotatedGeometry.current.center,
    2,
  );

  expect(rotated.pan.x).toBeCloseTo(237_500 / 350_000, 6);
  expect(rotated.pan.y).toBeCloseTo(-0.5, 6);
  expect(rotated.placement.center.x).toBeCloseTo(225_000, 6);
  expect(rotated.placement.center.y).toBeCloseTo(337_500, 6);
  expect(rotated.placement.size.width).toBeCloseTo(900_000, 6);
  expect(rotated.placement.size.height).toBeCloseTo(600_000, 6);
});

test("zooms a previewed placement around the Frame center", () => {
  const geometry = createPhotoGeometry(
    placementCases[0].expectedPlan,
  );

  // The Photo point at the Frame center stays put both ways.
  const zoomedIn = geometry.zoom(1.12);
  const back = geometry.zoom(1, {
    center: zoomedIn.placement.center,
    zoom: zoomedIn.zoom,
  });

  expect(back.pan.x).toBeCloseTo(-0.9, 9);
  expect(back.placement.center.x).toBeCloseTo(105_000, 6);
  expect(back.placement.size.width).toBeCloseTo(400_000, 6);
  // Zoom 1 leaves no vertical room, so the Photo is centered on that axis.
  expect(back.pan.y).toBe(0);
  expect(back.placement.center.y).toBeCloseTo(100_000, 6);

  // A dragged preview zooms from where the drag left it.
  const dragged = geometry.constrain({ x: 125_000, y: 100_000 }, 1.12);
  const zoomedDrag = geometry.zoom(1.5, {
    center: dragged.placement.center,
    zoom: dragged.zoom,
  });

  expect(zoomedDrag.zoom).toBeCloseTo(1.5, 9);
  expect(zoomedDrag.placement.center.x).toBeCloseTo(
    150_000 - (25_000 * 1.5) / 1.12,
    6,
  );
  expect(zoomedDrag.pan.x).toBeCloseTo(
    -((25_000 * 1.5) / 1.12) / 150_000,
    9,
  );
});

test("bounds the anchored Zoom to the Zoom range", () => {
  const geometry = createPhotoGeometry(
    placementCases[2].expectedPlan,
  );

  const zoomed = geometry.zoom(0.5);

  expect(zoomed.zoom).toBe(1);
  // -225 mm at Zoom 2 becomes -112.5 mm, beyond the 50 mm of room at Zoom 1.
  expect(zoomed.pan.x).toBe(-1);
  expect(zoomed.placement.center.x).toBeCloseTo(100_000, 6);
  expect(zoomed.placement.size.width).toBeCloseTo(400_000, 6);
});

function expectPlacementClose(
  actual: CanvasPhotoPlacement,
  expected: PhotoPlacement,
  caseName: string,
) {
  expect(
    actual.center.x,
    `${caseName}: center.x`,
  ).toBeCloseTo(expected.center.x, 4);
  expect(
    actual.center.y,
    `${caseName}: center.y`,
  ).toBeCloseTo(expected.center.y, 4);
  expect(
    actual.size.width,
    `${caseName}: size.width`,
  ).toBeCloseTo(expected.size.width, 4);
  expect(
    actual.size.height,
    `${caseName}: size.height`,
  ).toBeCloseTo(expected.size.height, 4);
}
