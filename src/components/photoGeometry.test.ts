import { expect, test } from "vitest";

import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type { PhotoPlacement, PhotoPlacementPlan, Vector2 } from "../domain/project";
import { createPhotoGeometry } from "./photoGeometry";

interface PanPreview {
  center: Vector2;
  expectedPan: Vector2;
  expectedPlacement: PhotoPlacement;
}

interface ZoomPreview {
  zoom: number;
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
      expectPlacementClose(
        geometry.zoom(preview.zoom).placement,
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

function expectPlacementClose(
  actual: PhotoPlacement,
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
