import { useState } from "react";

import { InspectorPanel } from "./components/InspectorPanel";
import type {
  AlbumInformation,
  ComposedSheet,
  DocumentSnapshot,
  MediaCatalogItem,
  ProjectedActiveSides,
  ProjectedVisualDefaults,
  SheetRole,
  SheetSnapshot,
} from "./domain/project";
import { mediaPanelPreviewFixture } from "./test/mediaPanelPreviewFixtures";
import { representativeProjection } from "./test/projectFixtures";

const sheetColors = [
  "#2b2823",
  "#efeae1",
  "#f7f4ef",
  "#e3ded4",
  "#efeae1",
  "#2b2823",
] as const;
const sheetPageNumbers = [[1], [2, 3], [4, 5], [6, 7], [8, 9], [10]] as const;

const initialSheetStates: readonly SheetSnapshot[] = sheetColors.map((_, index) => {
  const number = index + 1;
  const activeSides = activeSidesFor(number);
  return {
    activeSides,
    frames: [],
    heightUm: 300_000,
    id: `sheet-${String(number).padStart(3, "0")}`,
    number,
    pageNumbers: [...sheetPageNumbers[index]],
    role: roleFor(number),
    widthUm: activeSides === "both" ? 600_000 : 300_000,
  };
});

const sheets: readonly ComposedSheet[] = initialSheetStates.map((state, index) => {
  const source = representativeProjection.composition.sheets[0];
  const drawRect = {
    height: state.heightUm,
    width: state.widthUm,
    x: 0,
    y: 0,
  };
  const showPhoto = state.activeSides === "both" && index % 2 === 1;
  return {
    activeSides: state.activeSides,
    backgrounds: [
      {
        drawRect,
        kind: "color",
        rgb: sheetColors[index],
      },
    ],
    base: { drawRect, rgb: sheetColors[index] },
    frames: showPhoto
      ? source.frames.map((frame) => ({
          ...frame,
          frameId: `${frame.frameId}-${state.number}`,
        }))
      : [],
    heightUm: state.heightUm,
    number: state.number,
    overlays: [],
    sheetId: state.id,
    widthUm: state.widthUm,
  };
});

const previewUrl =
  mediaPanelPreviewFixture.mediaPreviews["test-media-001"]?.url;
const decorativePreview: MediaCatalogItem = {
  id: "decorative-preview-001",
  kind: "decorative",
  name: "Textura suave.svg",
  sourceWidthPx: 6_000,
  sourceHeightPx: 4_000,
  palette: ["#D5CCBE", "#E9E3D9", "#FAF8F4"],
};

export function SheetGridPreview() {
  const [focusedSheetId, setFocusedSheetId] = useState("sheet-002");
  const [document, setDocument] = useState<DocumentSnapshot>(
    representativeProjection.state.document,
  );
  const [sheetStates, setSheetStates] = useState(initialSheetStates);
  const [visualDefaults, setVisualDefaults] = useState<ProjectedVisualDefaults>(
    representativeProjection.state.album.visualDefaults,
  );

  function applyInformation(information: AlbumInformation) {
    setDocument({
      displayUnit: information.displayUnit,
      sheetWidthUm: information.sheetWidthUm,
      sheetHeightUm: information.sheetHeightUm,
      dpi: information.dpi,
      bleedUm: information.bleedUm,
      safetyUm: information.safetyUm,
    });
    setSheetStates((current) =>
      current.map((sheet, index) => ({
        ...sheet,
        activeSides:
          index === 0
            ? information.firstSheet === "double"
              ? "both"
              : "right"
            : index === current.length - 1
              ? information.lastSheet === "double"
                ? "both"
                : "left"
              : sheet.activeSides,
      })),
    );
  }

  return (
    <main className="sheet-grid-preview" data-development-preview="sheet-grid">
      <InspectorPanel
        displayedPhotoPanX={0}
        displayedPhotoZoom={1}
        document={document}
        frameBorder={visualDefaults.frameBorder}
        focusedSheetId={focusedSheetId}
        mediaItems={[
          ...representativeProjection.state.album.media,
          decorativePreview,
        ]}
        mediaPreviewUrls={
          previewUrl
            ? {
                "media-001": previewUrl,
                [decorativePreview.id]: previewUrl,
              }
            : {}
        }
        onApplyAlbumInformation={applyInformation}
        onApplyAlbumDesign={setVisualDefaults}
        onBeginPhotoZoom={() => undefined}
        onFinishPhotoZoom={async () => undefined}
        onNavigateToSheet={setFocusedSheetId}
        onUpdatePhotoZoom={() => undefined}
        onValidateAlbumInformation={async () => ({
          errors: [],
          impact: {
            sheetWidthPx: 7_087,
            pageWidthPx: 3_543,
            heightPx: 3_543,
          },
        })}
        selectedComposedPhoto={null}
        selectedFrame={null}
        sheetStates={sheetStates}
        sheets={sheets}
        visualDefaults={visualDefaults}
        zoomCommitting={false}
      />
    </main>
  );
}

function activeSidesFor(number: number): ProjectedActiveSides {
  if (number === 1) return "right";
  if (number === sheetColors.length) return "left";
  return "both";
}

function roleFor(number: number): SheetRole {
  if (number === 1) return "initial";
  if (number === sheetColors.length) return "final";
  return "internal";
}
