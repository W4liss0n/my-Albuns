import { useCallback, useMemo, useState } from "react";

import { InspectorPanel } from "./components/InspectorPanel";
import type {
  AlbumInformation,
  ComposedBackground,
  ComposedDecorative,
  ComposedSheet,
  DisplayUnit,
  DocumentSnapshot,
  MediaCatalogItem,
  ProjectedActiveSides,
  ProjectedBackgroundContent,
  ProjectedOverlayContent,
  ProjectedVisualDefaults,
  RectUm,
  SheetRole,
  SheetSnapshot,
} from "./domain/project";
import { mediaPanelPreviewFixture } from "./test/mediaPanelPreviewFixtures";
import { representativeProjection } from "./test/projectFixtures";

const PREVIEW_SHEET_COUNT = 6;

const initialSheetStates: readonly SheetSnapshot[] = renumberSheetStates(
  Array.from({ length: PREVIEW_SHEET_COUNT }, (_, index) => {
    const number = index + 1;
    const activeSides = activeSidesFor(number);
    return {
      activeSides,
      frames: [],
      heightUm: 300_000,
      id: `sheet-${String(number).padStart(3, "0")}`,
      number,
      pageNumbers: [],
      role: roleFor(number),
      widthUm: activeSides === "both" ? 600_000 : 300_000,
    };
  }),
);

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
  const mediaItems = useMemo(
    () => [...representativeProjection.state.album.media, decorativePreview],
    [],
  );
  const sheets = useMemo(
    () => composePreviewSheets(sheetStates, visualDefaults, mediaItems),
    [mediaItems, sheetStates, visualDefaults],
  );
  const [presentationUnitOverride, setPresentationUnitOverride] =
    useState<DisplayUnit | null>(null);
  const changePresentationUnit = useCallback((unit: DisplayUnit | null) => {
    setPresentationUnitOverride(unit);
  }, []);
  const presentationUnit = presentationUnitOverride ?? document.displayUnit;

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
      renumberSheetStates(
        current.map((sheet, index) => {
          const activeSides =
            index === 0
              ? information.firstSheet === "double"
                ? "both"
                : "right"
              : index === current.length - 1
                ? information.lastSheet === "double"
                  ? "both"
                  : "left"
                : sheet.activeSides;
          return {
            ...sheet,
            activeSides,
            heightUm: information.sheetHeightUm,
            widthUm:
              activeSides === "both"
                ? information.sheetWidthUm
                : information.sheetWidthUm / 2,
          };
        }),
      ),
    );
  }

  return (
    <main className="sheet-grid-preview" data-development-preview="sheet-grid">
      <InspectorPanel
        context={{ kind: "album" }}
        displayedPhotoPanX={0}
        displayedPhotoZoom={1}
        document={document}
        presentationUnit={presentationUnit}
        frameBorder={visualDefaults.frameBorder}
        focusedSheetId={focusedSheetId}
        mediaItems={mediaItems}
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
        onPresentationUnitChange={changePresentationUnit}
        onUpdatePhotoZoom={() => undefined}
        onValidateAlbumInformation={async () => ({
          errors: [],
          impact: {
            sheetWidthPx: 7_087,
            pageWidthPx: 3_543,
            heightPx: 3_543,
          },
        })}
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
  if (number === PREVIEW_SHEET_COUNT) return "left";
  return "both";
}

function roleFor(number: number): SheetRole {
  if (number === 1) return "initial";
  if (number === PREVIEW_SHEET_COUNT) return "final";
  return "internal";
}

function renumberSheetStates(
  sheets: readonly SheetSnapshot[],
): SheetSnapshot[] {
  let nextPageNumber = 1;
  return sheets.map((sheet) => {
    const pageCount = sheet.activeSides === "both" ? 2 : 1;
    const pageNumbers = Array.from(
      { length: pageCount },
      () => nextPageNumber++,
    );
    return { ...sheet, pageNumbers };
  });
}

function composePreviewSheets(
  states: readonly SheetSnapshot[],
  visualDefaults: ProjectedVisualDefaults,
  mediaItems: readonly MediaCatalogItem[],
): readonly ComposedSheet[] {
  const source = representativeProjection.composition.sheets[0];
  const mediaNames = new Map(mediaItems.map((media) => [media.id, media.name]));

  return states.map((state, index) => {
    const drawRect = rect(0, 0, state.widthUm, state.heightUm);
    const showPhoto = state.activeSides === "both" && index % 2 === 1;
    const scaleX = state.widthUm / source.widthUm;
    const scaleY = state.heightUm / source.heightUm;
    return {
      activeSides: state.activeSides,
      backgrounds: composeBackgrounds(
        visualDefaults,
        state.activeSides,
        drawRect,
        mediaNames,
      ),
      base: { drawRect, rgb: "#FFFFFF" },
      frames: showPhoto
        ? source.frames.map((frame) => ({
            ...frame,
            frameId: `${frame.frameId}-${state.number}`,
            clipRect: scaleRect(frame.clipRect, scaleX, scaleY),
            borderFillRects: frame.borderFillRects.map((border) =>
              scaleRect(border, scaleX, scaleY),
            ),
            photo: frame.photo
              ? {
                  ...frame.photo,
                  drawRect: scaleRect(frame.photo.drawRect, scaleX, scaleY),
                }
              : null,
          }))
        : [],
      heightUm: state.heightUm,
      number: state.number,
      overlays: composeOverlays(
        visualDefaults,
        state.activeSides,
        drawRect,
        mediaNames,
      ),
      sheetId: state.id,
      widthUm: state.widthUm,
    };
  });
}

function composeBackgrounds(
  defaults: ProjectedVisualDefaults,
  activeSides: ProjectedActiveSides,
  drawRect: RectUm,
  mediaNames: ReadonlyMap<string, string>,
): ComposedBackground[] {
  if (defaults.background.scope === "bothSides") {
    return [backgroundLayer(defaults.background.both, drawRect, mediaNames)];
  }
  if (activeSides !== "both") {
    return [
      backgroundLayer(
        defaults.background[activeSides],
        drawRect,
        mediaNames,
      ),
    ];
  }
  const halfWidth = drawRect.width / 2;
  return [
    backgroundLayer(
      defaults.background.left,
      rect(drawRect.x, drawRect.y, halfWidth, drawRect.height),
      mediaNames,
    ),
    backgroundLayer(
      defaults.background.right,
      rect(drawRect.x + halfWidth, drawRect.y, halfWidth, drawRect.height),
      mediaNames,
    ),
  ];
}

function composeOverlays(
  defaults: ProjectedVisualDefaults,
  activeSides: ProjectedActiveSides,
  drawRect: RectUm,
  mediaNames: ReadonlyMap<string, string>,
): ComposedDecorative[] {
  if (defaults.overlay.scope === "bothSides") {
    return overlayLayer(defaults.overlay.both, drawRect, mediaNames);
  }
  if (activeSides !== "both") {
    return overlayLayer(defaults.overlay[activeSides], drawRect, mediaNames);
  }
  const halfWidth = drawRect.width / 2;
  return [
    ...overlayLayer(
      defaults.overlay.left,
      rect(drawRect.x, drawRect.y, halfWidth, drawRect.height),
      mediaNames,
    ),
    ...overlayLayer(
      defaults.overlay.right,
      rect(drawRect.x + halfWidth, drawRect.y, halfWidth, drawRect.height),
      mediaNames,
    ),
  ];
}

function backgroundLayer(
  content: ProjectedBackgroundContent,
  drawRect: RectUm,
  mediaNames: ReadonlyMap<string, string>,
): ComposedBackground {
  return content.kind === "color"
    ? { drawRect, kind: "color", rgb: content.rgb }
    : {
        drawRect,
        kind: "media",
        mediaId: content.mediaId,
        name: mediaNames.get(content.mediaId) ?? "Decorativo",
      };
}

function overlayLayer(
  content: ProjectedOverlayContent | null,
  drawRect: RectUm,
  mediaNames: ReadonlyMap<string, string>,
): ComposedDecorative[] {
  return content
    ? [
        {
          drawRect,
          mediaId: content.mediaId,
          name: mediaNames.get(content.mediaId) ?? "Decorativo",
        },
      ]
    : [];
}

function scaleRect(source: RectUm, scaleX: number, scaleY: number): RectUm {
  return rect(
    source.x * scaleX,
    source.y * scaleY,
    source.width * scaleX,
    source.height * scaleY,
  );
}

function rect(x: number, y: number, width: number, height: number): RectUm {
  return { height, width, x, y };
}
