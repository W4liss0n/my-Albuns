import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  CompositionPlan,
  PhotoPlacementPlan,
} from "../domain/project";

export const composition: CompositionPlan = {
  sheets: [
    {
      sheetId: "sheet-001",
      number: 1,
      activeSides: "both",
      widthUm: 600_000,
      heightUm: 300_000,
      overlay: null,
      frames: [],
    },
  ],
};
export const threeSheetComposition: CompositionPlan = {
  sheets: [1, 2, 3].map((number) => ({
    sheetId: `sheet-00${number}`,
    number,
    activeSides: "both" as const,
    widthUm: 600_000,
    heightUm: 300_000,
    overlay: null,
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
  panToCenterPerZoom: {
    xx: 200_000,
    xy: 0,
    yx: 0,
    yy: 100_000,
  },
  sizePerZoom: { width: 400_000, height: 200_000 },
};

export const interactiveComposition: CompositionPlan = {
  sheets: [
    {
      sheetId: "sheet-001",
      number: 1,
      activeSides: "both",
      widthUm: 600_000,
      heightUm: 300_000,
      overlay: null,
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

export const pannedInteractiveComposition: CompositionPlan = {
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

export const rotatedInteractiveComposition: CompositionPlan = {
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
