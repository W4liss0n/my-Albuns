import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  EditorProjection,
  PhotoPlacementPlan,
} from "../domain/project";

export const representativeProjection: EditorProjection = {
  state: {
    projectId: "project-spike-001",
    projectName: "Álbum Horizonte",
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
          widthUm: 600_000,
          heightUm: 300_000,
          hasOverlay: false,
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
                name: "Serra ao amanhecer.jpg",
                sourceWidthPx: 6_000,
                sourceHeightPx: 4_000,
                palette: ["#10202b", "#648493", "#dfa75e"],
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
          name: "Serra ao amanhecer.jpg",
          sourceWidthPx: 6_000,
          sourceHeightPx: 4_000,
          palette: ["#10202b", "#648493", "#dfa75e"],
        },
        {
          id: "media-002",
          name: "Campo.jpg",
          sourceWidthPx: 4_000,
          sourceHeightPx: 6_000,
          palette: ["#21372f", "#92a277", "#e5d7b9"],
        },
        {
          id: "media-003",
          name: "Praia.jpg",
          sourceWidthPx: 6_000,
          sourceHeightPx: 4_000,
          palette: ["#123e52", "#428596", "#e7bd76"],
        },
      ],
    },
  },
  composition: {
    sheets: [
      {
        sheetId: "sheet-001",
        number: 1,
        widthUm: 600_000,
        heightUm: 300_000,
        hasOverlay: false,
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
      },
    },
    composition: {
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
