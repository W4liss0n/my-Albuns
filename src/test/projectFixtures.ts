import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  EditorProjection,
  PhotoPlacementPlan,
} from "../domain/project";

export const representativeProjection: EditorProjection = {
  state: {
    projectId: "project-spike-001",
    projectName: "Álbum Horizonte",
    document: {
      displayUnit: "mm",
      sheetWidthUm: 600_000,
      sheetHeightUm: 300_000,
      dpi: 300,
      bleedUm: 3_000,
      safetyUm: 3_000,
    },
    revision: 25,
    savedRevision: 0,
    dirty: true,
    canUndo: true,
    canRedo: false,
    album: {
      sheets: [
        {
          id: "sheet-001",
          number: 1,
          role: "initial",
          activeSides: "both",
          widthUm: 600_000,
          heightUm: 300_000,
          frames: [
            {
              id: "frame-001",
              rect: {
                x: 20_000,
                y: 20_000,
                width: 280_000,
                height: 260_000,
              },
              zIndex: 0,
              photo: {
                mediaId: "media-001",
                transform: {
                  panX: 0,
                  panY: 0,
                  userZoom: 1,
                  quarterTurns: 0,
                  fineRotationDegrees: 0,
                  mirrorX: false,
                },
              },
            },
          ],
        },
      ],
      media: [
        {
          id: "media-001",
          kind: "photo",
          name: "Serra ao amanhecer.jpg",
          sourceWidthPx: 6_000,
          sourceHeightPx: 4_000,
          palette: ["#10202b", "#648493", "#dfa75e"],
        },
        {
          id: "media-002",
          kind: "photo",
          name: "Campo.jpg",
          sourceWidthPx: 4_000,
          sourceHeightPx: 6_000,
          palette: ["#21372f", "#92a277", "#e5d7b9"],
        },
        {
          id: "media-003",
          kind: "photo",
          name: "Praia.jpg",
          sourceWidthPx: 6_000,
          sourceHeightPx: 4_000,
          palette: ["#123e52", "#428596", "#e7bd76"],
        },
      ],
      visualDefaults: {
        background: {
          scope: "bothSides",
          both: { kind: "color", rgb: "#FFFFFF" },
        },
        overlay: { scope: "bothSides", both: null },
        frameBorder: { kind: "none" },
      },
    },
  },
  composition: {
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
        frames: [
          {
            frameId: "frame-001",
            clipRect: {
              x: 20_000,
              y: 20_000,
              width: 280_000,
              height: 260_000,
            },
            zIndex: 0,
            photo: {
              mediaId: "media-001",
              name: "Serra ao amanhecer.jpg",
              drawRect: {
                x: -50_000,
                y: 20_000,
                width: 400_000,
                height: 260_000,
              },
              placement: placementFixture.cases[0]
                .expectedPlan as PhotoPlacementPlan,
              rotationDegrees: 0,
              mirrorX: false,
              palette: ["#10202b", "#648493", "#dfa75e"],
            },
          },
        ],
        overlays: [],
      },
    ],
  },
  mediaUsage: [
    { mediaId: "media-001", count: 1 },
    { mediaId: "media-002", count: 0 },
    { mediaId: "media-003", count: 0 },
  ],
};

export function createEmptyProjection(): EditorProjection {
  return {
    state: {
      ...representativeProjection.state,
      revision: 0,
      savedRevision: 0,
      dirty: false,
      canUndo: false,
      album: {
        sheets: [],
        media: [],
        visualDefaults: representativeProjection.state.album.visualDefaults,
      },
    },
    composition: {
      frameBorder: { kind: "none" },
      sheets: [],
    },
    mediaUsage: [],
  };
}

export function createTwoSheetProjection(): EditorProjection {
  return {
    state: {
      ...representativeProjection.state,
      album: {
        ...representativeProjection.state.album,
        sheets: [
          representativeProjection.state.album.sheets[0],
          {
            ...representativeProjection.state.album.sheets[0],
            id: "sheet-002",
            number: 2,
            role: "final",
            frames: [],
          },
        ],
      },
    },
    composition: {
      frameBorder: representativeProjection.composition.frameBorder,
      sheets: [
        representativeProjection.composition.sheets[0],
        {
          ...representativeProjection.composition.sheets[0],
          sheetId: "sheet-002",
          number: 2,
          frames: [],
        },
      ],
    },
    mediaUsage: representativeProjection.mediaUsage,
  };
}
