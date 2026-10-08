import placementFixture from "../../../tests/fixtures/photo-placement-cases.json";
import type {
  CompositionPlan,
  PhotoPlacementPlan,
} from "../../domain/project";

export const composition: CompositionPlan = {
  frameBorder: { kind: "none" },
  sheets: [
    {
      sheetId: "sheet-001",
      number: 1,
      activeSides: "both",
      widthUm: 600_000,
      heightUm: 300_000,
      base: {
        rgb: "#FFFFFF",
        drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
      },
      backgrounds: [
        {
          kind: "color",
          rgb: "#FFFFFF",
          drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
        },
      ],
      overlays: [],
      frames: [],
    },
  ],
};
export const threeSheetComposition: CompositionPlan = {
  frameBorder: { kind: "none" },
  sheets: [1, 2, 3].map((number) => ({
    sheetId: `sheet-00${number}`,
    number,
    activeSides: "both" as const,
    widthUm: 600_000,
    heightUm: 300_000,
    base: {
      rgb: "#FFFFFF",
      drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
    },
    backgrounds: [
      {
        kind: "color" as const,
        rgb: "#FFFFFF",
        drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
      },
    ],
    overlays: [],
    frames: [],
  })),
};

const horizontalPlacementPlan: PhotoPlacementPlan = {
  baseFillZoom: 66.66666666666667,
  currentPan: { x: 0, y: 0 },
  currentZoom: 1,
  panRange: { minimum: -1, maximum: 1 },
  zoomRange: { minimum: 1, maximum: 5 },
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
  frameBorder: { kind: "none" },
  sheets: [
    {
      sheetId: "sheet-001",
      number: 1,
      activeSides: "both",
      widthUm: 600_000,
      heightUm: 300_000,
      base: {
        rgb: "#FFFFFF",
        drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
      },
      backgrounds: [
        {
          kind: "color",
          rgb: "#FFFFFF",
          drawRect: { x: 0, y: 0, width: 600_000, height: 300_000 },
        },
      ],
      overlays: [],
      frames: [
        {
          frameId: "frame-001",
          clipRect: {
            x: 0,
            y: 0,
            width: 300_000,
            height: 200_000,
          },
          border: { kind: "none" as const }, opacityByte: 255,
          borderFillRects: [],
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
            blackAndWhite: false,
            palette: ["#10202b", "#648493", "#dfa75e"],
          },
        },
      ],
    },
  ],
};

export const pannedInteractiveComposition: CompositionPlan = {
  frameBorder: { kind: "none" },
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

/**
 * The landscape Photo of `interactiveComposition` as the Core composes it at
 * `zoom`, its center `offsetXUm` right of the Frame center. The 400 x 200 mm
 * Photo grows 200 x 100 mm per unit of Zoom over the 300 x 200 mm Frame.
 */
export function landscapePhotoPlacement(
  zoom: number,
  offsetXUm: number,
): PhotoPlacementPlan {
  const roomXUm = 50_000 + 200_000 * (zoom - 1);
  return {
    ...horizontalPlacementPlan,
    currentPan: { x: offsetXUm / roomXUm, y: 0 },
    currentZoom: zoom,
    current: {
      center: { x: 150_000 + offsetXUm, y: 100_000 },
      size: { width: 400_000 * zoom, height: 200_000 * zoom },
    },
    panToCenter: { xx: roomXUm, xy: 0, yx: 0, yy: 100_000 * (zoom - 1) },
  };
}

/** Replaces the placement of one Frame's Photo, as a Core commit would. */
export function withPhotoPlacement(
  plan: CompositionPlan,
  frameId: string,
  placement: PhotoPlacementPlan,
): CompositionPlan {
  return {
    ...plan,
    sheets: plan.sheets.map((sheet) => ({
      ...sheet,
      frames: sheet.frames.map((frame) =>
        frame.frameId === frameId && frame.photo
          ? {
              ...frame,
              photo: {
                ...frame.photo,
                drawRect: {
                  x: frame.clipRect.x + placement.current.center.x -
                    placement.current.size.width / 2,
                  y: frame.clipRect.y + placement.current.center.y -
                    placement.current.size.height / 2,
                  width: placement.current.size.width,
                  height: placement.current.size.height,
                },
                placement,
              },
            }
          : frame,
      ),
    })),
  };
}

// frame-001 holds the left page's Photo and frame-002 the right page's.
export const twoFrameInteractiveComposition: CompositionPlan = {
  frameBorder: { kind: "none" },
  sheets: [
    {
      ...interactiveComposition.sheets[0],
      frames: [
        interactiveComposition.sheets[0].frames[0],
        {
          ...interactiveComposition.sheets[0].frames[0],
          frameId: "frame-002",
          clipRect: { x: 300_000, y: 0, width: 300_000, height: 200_000 },
          zIndex: 1,
          photo: {
            ...interactiveComposition.sheets[0].frames[0].photo!,
            mediaId: "media-002",
            name: "Praia.jpg",
            drawRect: { x: 250_000, y: 0, width: 400_000, height: 200_000 },
          },
        },
      ],
    },
  ],
};

export const rotatedInteractiveComposition: CompositionPlan = {
  frameBorder: { kind: "none" },
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
