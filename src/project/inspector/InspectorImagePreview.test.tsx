import { fireEvent, render, screen, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import type { MediaCatalogItem } from "../../domain/project";
import { rasterLimitsAt300Dpi } from "../../test/projectConfigurationFixtures";
import { representativeProjection } from "../../test/projectFixtures";
import { InspectorPanel, type InspectorContext } from "./InspectorPanel";

const composedSheet = representativeProjection.composition.sheets[0];
const photoFrame = representativeProjection.state.album.sheets[0].frames[0];
const [landscape, portrait] = representativeProjection.state.album.media;
const LANDSCAPE_URL = "data:image/png;base64,TEFORFNDQVBF";
const PORTRAIT_URL = "data:image/png;base64,UE9SVFJBSVQ=";

function renderInspector(
  context: InspectorContext,
  overrides: Partial<ComponentProps<typeof InspectorPanel>> = {},
) {
  const props: ComponentProps<typeof InspectorPanel> = {
    context,
    displayedPhotoZoom: 1,
    document: representativeProjection.state.document,
    focusedSheetId: composedSheet.sheetId,
    frameGapUm: 5_000,
    mediaItems: representativeProjection.state.album.media,
    mediaPreviews: {
      [landscape.id]: { mediaId: landscape.id, state: "ready", url: LANDSCAPE_URL },
    },
    onApplyAlbumDesign: vi.fn(),
    onApplyAlbumInformation: vi.fn(),
    onNavigateToSheet: vi.fn(),
    onPresentationUnitChange: vi.fn(),
    onSelectSheet: vi.fn(),
    onValidateAlbumInformation: vi.fn(async () => ({
      rasterLimits: rasterLimitsAt300Dpi,
      errors: [],
      impact: { conversionLosses: [], heightPx: 3_543, pageWidthPx: 3_543, sheetWidthPx: 7_087 },
    })),
    presentationUnit: representativeProjection.state.document.displayUnit,
    revision: representativeProjection.state.revision,
    sectionState: { kind: "local" },
    sheetStates: representativeProjection.state.album.sheets,
    sheets: representativeProjection.composition.sheets,
    visualDefaults: representativeProjection.state.album.visualDefaults,
    ...overrides,
  };
  const view = render(<InspectorPanel {...props} />);
  return {
    ...view,
    rerenderInspector(next: InspectorContext, nextOverrides: Partial<ComponentProps<typeof InspectorPanel>> = {}) {
      view.rerender(<InspectorPanel {...props} {...nextOverrides} context={next} />);
    },
  };
}

function previewImages() {
  return Array.from(document.querySelectorAll<HTMLImageElement>(".inspector-image-preview img"));
}

test("a Frame with a Photo shows only its whole preview between the heading and Design", () => {
  renderInspector({ kind: "frame", frame: photoFrame, composedPhoto: composedSheet.frames[0].photo });

  const heading = screen.getByRole("heading", { name: "Serra ao amanhecer.jpg" });
  const preview = screen.getByRole("img", { name: "Prévia de Serra ao amanhecer.jpg" });
  const design = screen.getByRole("button", { name: "Design" });
  expect(heading.compareDocumentPosition(preview) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(preview.compareDocumentPosition(design) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  // Fixed: no section header and nothing to collapse.
  expect(preview.closest("section")).toBeNull();
  expect(previewImages().map((image) => image.getAttribute("src"))).toEqual([LANDSCAPE_URL]);
  expect(previewImages()[0]).toHaveAttribute("loading", "eager");
  expect(preview.querySelector(".media-preview-thumbnail")).toHaveStyle({ "--media-aspect-ratio": "6000 / 4000" });
  // The author left the pixel size out: the block holds the image alone.
  expect(document.querySelector(".inspector-image-preview")!.textContent).toBe("");
});

test("an empty Frame and several Frames have no preview", () => {
  const view = renderInspector({ kind: "frame", frame: { ...photoFrame, photo: null }, composedPhoto: null });
  expect(screen.getByRole("heading", { name: "Quadro vazio" })).toBeInTheDocument();
  expect(document.querySelector(".inspector-image-preview")).toBeNull();

  view.rerenderInspector({
    kind: "multiple-frames",
    frames: [photoFrame, { ...photoFrame, id: "frame-002" }],
    editingSheet: composedSheet,
  });
  expect(screen.getByRole("heading", { name: "2 quadros selecionados" })).toBeInTheDocument();
  expect(document.querySelector(".inspector-image-preview")).toBeNull();
});

test("a selected image replaces the Album with its heading and preview", () => {
  renderInspector({ kind: "media", media: landscape });

  expect(screen.getByText("Imagem selecionada")).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Serra ao amanhecer.jpg" })).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "Prévia de Serra ao amanhecer.jpg" })).toBeInTheDocument();
  expect(previewImages().map((image) => image.getAttribute("src"))).toEqual([LANDSCAPE_URL]);
  expect(document.querySelector(".inspector-image-preview")!.textContent).toBe("");
  for (const name of ["Informações do álbum", "Design do álbum", "Grade de lâminas", "Design da lâmina", "Design"]) {
    expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
  }
});

test("a Decorative has a preview, and a Photo without observed metadata takes the loaded proportion", () => {
  const decorative: MediaCatalogItem = {
    id: "decorative-001", kind: "decorative", name: "Textura.png", palette: null,
    sourceWidthPx: null, sourceHeightPx: null,
  };
  const view = renderInspector({ kind: "media", media: decorative });
  expect(screen.getByRole("heading", { name: "Textura.png" })).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "Prévia de Textura.png" })).toBeInTheDocument();

  view.rerenderInspector({ kind: "media", media: { ...portrait, sourceWidthPx: 1, sourceHeightPx: 1 } }, {
    mediaPreviews: { [portrait.id]: { mediaId: portrait.id, state: "ready", url: PORTRAIT_URL } },
  });
  expect(screen.getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  // The loaded preview, not the placeholder 1 × 1, gives the proportion.
  loadPreview();
  expect(document.querySelector(".media-preview-thumbnail")).toHaveStyle({ "--media-aspect-ratio": "1200 / 800" });
});

test("several selected images show only a heading", () => {
  renderInspector({ kind: "multiple-media", count: 3 });
  expect(screen.getByText("Seleção múltipla")).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "3 imagens selecionadas" })).toBeInTheDocument();
  expect(document.querySelector(".inspector-image-preview")).toBeNull();
  expect(screen.queryByRole("button", { name: "Informações do álbum" })).not.toBeInTheDocument();
});

test("a missing file shows the media panel symbol without a notice, and a retained preview silently", () => {
  const view = renderInspector({ kind: "media", media: portrait }, {
    mediaPreviews: { [portrait.id]: { mediaId: portrait.id, state: "absent", url: null } },
    missingMediaIds: new Set([portrait.id]),
  });
  const preview = screen.getByRole("img", { name: "Arquivo ausente" });
  expect(preview.querySelector(".media-preview-thumbnail")).toHaveAttribute("data-missing", "true");
  expect(preview.querySelector(".media-preview-thumbnail__missing-symbol")).not.toBeNull();
  expect(previewImages()).toEqual([]);
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.queryByRole("button", { name: /Localizar/ })).not.toBeInTheDocument();

  view.rerenderInspector({ kind: "media", media: portrait }, {
    mediaPreviews: { [portrait.id]: { mediaId: portrait.id, state: "absent", url: PORTRAIT_URL } },
    missingMediaIds: new Set(),
  });
  expect(screen.getByRole("img", { name: "Prévia de Campo.jpg" })).toBeInTheDocument();
  expect(previewImages().map((image) => image.getAttribute("src"))).toEqual([PORTRAIT_URL]);
});

test("switching images never shows the previous image under the new name", () => {
  const both = {
    mediaPreviews: {
      [landscape.id]: { mediaId: landscape.id, state: "ready" as const, url: LANDSCAPE_URL },
      [portrait.id]: { mediaId: portrait.id, state: "ready" as const, url: PORTRAIT_URL },
    },
  };
  const view = renderInspector({ kind: "media", media: landscape }, both);
  loadPreview();

  // The next preview is still loading: the media panel thumbnail would keep
  // the previous image on screen until then; the contextual panel must not.
  view.rerenderInspector({ kind: "media", media: portrait }, both);
  expect(screen.getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  expect(previewImages().map((image) => image.getAttribute("src"))).toEqual([PORTRAIT_URL]);
  expect(previewImages()[0]).not.toHaveAttribute("data-pending");

  view.rerenderInspector({ kind: "frame", frame: photoFrame, composedPhoto: composedSheet.frames[0].photo }, both);
  loadPreview();
  view.rerenderInspector({
    kind: "frame",
    frame: { ...photoFrame, id: "frame-002", photo: { ...photoFrame.photo!, mediaId: portrait.id } },
    composedPhoto: { ...composedSheet.frames[0].photo!, mediaId: portrait.id, name: portrait.name },
  }, both);
  expect(previewImages().map((image) => image.getAttribute("src"))).toEqual([PORTRAIT_URL]);
});

function loadPreview() {
  const [image] = previewImages();
  Object.defineProperty(image, "naturalWidth", { configurable: true, value: 1_200 });
  Object.defineProperty(image, "naturalHeight", { configurable: true, value: 800 });
  fireEvent.load(image);
}

test("any context borrowing the panel from the Album keeps its drafts and the Grade's tiles", () => {
  const onPresentationUnitChange = vi.fn();
  const view = renderInspector({ kind: "album" }, { onPresentationUnitChange });
  const grid = document.querySelector(".sheet-grid");
  expect(grid).not.toBeNull();
  const applyDesign = () => within(
    screen.getByRole("button", { name: "Design do álbum" }).closest("section")!,
  ).getByRole("button", { name: "Aplicar" });
  fireEvent.change(screen.getByRole("slider", { name: "Espaço entre quadros" }), { target: { value: "6000" } });
  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), { target: { value: "in" } });
  expect(applyDesign()).toBeEnabled();
  expect(onPresentationUnitChange).toHaveBeenLastCalledWith("in");

  for (const context of [
    { kind: "media", media: landscape },
    { kind: "multiple-media", count: 2 },
    { kind: "frame", frame: photoFrame, composedPhoto: composedSheet.frames[0].photo },
    { kind: "sheet", sheet: composedSheet },
  ] satisfies InspectorContext[]) {
    view.rerenderInspector(context, { onPresentationUnitChange });
    expect(screen.queryByRole("button", { name: "Design do álbum" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Grade de lâminas" })).not.toBeInTheDocument();
    // Hidden, the form no longer converts the window to its pending Unit.
    expect(onPresentationUnitChange).toHaveBeenLastCalledWith(null);

    view.rerenderInspector({ kind: "album" }, { onPresentationUnitChange });
    // New tiles would load their Cache previews again and fill in one by one.
    expect(document.querySelector(".sheet-grid")).toBe(grid);
    expect(screen.getByRole("slider", { name: "Espaço entre quadros" })).toHaveValue("6000");
    expect(screen.getByRole("combobox", { name: "Unidade" })).toHaveValue("in");
    expect(onPresentationUnitChange).toHaveBeenLastCalledWith("in");
    expect(applyDesign()).toBeEnabled();
  }
});

test("an image borrowing the panel during Sheet editing keeps the Sheet's chosen scope", () => {
  const view = renderInspector({ kind: "sheet", sheet: composedSheet });
  fireEvent.click(screen.getByRole("button", { name: "Página esquerda" }));

  view.rerenderInspector({ kind: "media", media: landscape, editingSheet: composedSheet });
  expect(screen.queryByRole("button", { name: "Design da lâmina" })).not.toBeInTheDocument();

  view.rerenderInspector({ kind: "sheet", sheet: composedSheet });
  expect(screen.getByRole("button", { name: "Página esquerda" })).toHaveAttribute("aria-pressed", "true");
});
