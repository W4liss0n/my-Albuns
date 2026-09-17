import placementFixture from "../../tests/fixtures/photo-placement-cases.json";
import type {
  EditorProjection,
  SheetSnapshot,
  PhotoPlacementPlan,
} from "../domain/project";

export const representativeProjection: EditorProjection = {
  canPasteFrames: false,
  state: {
    layoutSettings: { permission: "pagesAndSheet", marginUm: 15_000, gapUm: 5_000, minimumSideUm: 20_000 },
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
          structure: { availability: { canAddBefore: true, canAddAfter: true, canConvertEdge: true, canDelete: false, canDuplicate: true }, minimumReorderIndex: 0, maximumReorderIndex: 0 },
          edgeConversionLoss: null,
          id: "sheet-001",
          number: 1,
          role: "initial",
          activeSides: "both",
          pageNumbers: [1, 2],
          layoutLocked: false,
          widthUm: 600_000,
          heightUm: 300_000,
          frames: [
            {
              id: "frame-001",
              style: { source: "album", borderRgb: "#000000", borderWidthUm: 0, opacityPercent: 100 },
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
                  blackAndWhite: false,
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
            border: { kind: "none" as const }, opacityByte: 255,
            borderFillRects: [],
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
              blackAndWhite: false,
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
    canPasteFrames: false,
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
  const projection: EditorProjection = {
    canPasteFrames: false,
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
            pageNumbers: [3, 4],
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
  projection.state.album.sheets = refreshSheetStructureFixture(projection.state.album.sheets);
  return projection;
}

export function createThreeSheetProjection(): EditorProjection {
  const projection = structuredClone(createTwoSheetProjection());
  const secondSheet = projection.state.album.sheets[1];
  const secondComposition = projection.composition.sheets[1];
  if (!secondSheet || !secondComposition) return projection;

  secondSheet.role = "internal";
  const finalSheet = {
    ...structuredClone(secondSheet),
    id: "sheet-003",
    number: 3,
    role: "final" as const,
    pageNumbers: [5, 6],
    frames: [],
  };
  const finalComposition = {
    ...structuredClone(secondComposition),
    sheetId: "sheet-003",
    number: 3,
    frames: [],
  };
  projection.state.album.sheets.push(finalSheet);
  projection.composition.sheets.push(finalComposition);
  projection.state.album.sheets = refreshSheetStructureFixture(projection.state.album.sheets);
  return projection;
}

/** Deterministic fake-port facts only. Production consumes the Rust projection.
 * Public Core tests prove the structure/command contract independently. */
export function refreshSheetStructureFixture(
  sheets: readonly (Omit<SheetSnapshot, "structure" | "edgeConversionLoss"> & Partial<Pick<SheetSnapshot, "structure" | "edgeConversionLoss">>)[],
): SheetSnapshot[] {
  const last = sheets.length - 1;
  return sheets.map((sheet, index) => ({ ...sheet, edgeConversionLoss: sheet.edgeConversionLoss ?? null,
    structure: {
      availability: { canAddBefore: index > 0 || sheet.activeSides === "both",
        canAddAfter: index < last || sheet.activeSides === "both",
        canConvertEdge: index === 0 || index === last, canDelete: sheets.length > 2,
        canDuplicate: sheet.activeSides === "both" },
      minimumReorderIndex: sheet.activeSides === "both" ? Number(sheets[0].activeSides !== "both") : index,
      maximumReorderIndex: sheet.activeSides === "both" ? last - Number(sheets[last].activeSides !== "both") : index,
    },
  }));
}
