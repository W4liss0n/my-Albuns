// @ts-expect-error Node is available in Vitest but excluded from frontend types.
import { readFileSync } from "node:fs";

import { act, fireEvent, render, screen, within } from "@testing-library/react";
import type { ComponentProps } from "react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";

import type { MediaCatalogItem } from "../../domain/project";
import { rasterLimitsAt300Dpi } from "../../test/projectConfigurationFixtures";
import { representativeProjection } from "../../test/projectFixtures";
import { INSPECTOR_PREVIEW_SWAP_WAIT_MS } from "./InspectorImagePreview";
import { InspectorPanel, type InspectorContext } from "./InspectorPanel";

const composedSheet = representativeProjection.composition.sheets[0];
const photoFrame = representativeProjection.state.album.sheets[0].frames[0];
const [landscape, portrait, beach] = representativeProjection.state.album.media;
const LANDSCAPE_URL = "data:image/png;base64,TEFORFNDQVBF";
const PORTRAIT_URL = "data:image/png;base64,UE9SVFJBSVQ=";
const BEACH_URL = "data:image/png;base64,UFJBSUE=";
const ALL_READY = {
  mediaPreviews: {
    [landscape.id]: { mediaId: landscape.id, state: "ready" as const, url: LANDSCAPE_URL },
    [portrait.id]: { mediaId: portrait.id, state: "ready" as const, url: PORTRAIT_URL },
    [beach.id]: { mediaId: beach.id, state: "ready" as const, url: BEACH_URL },
  },
};
const landscapeFrame: InspectorContext = {
  kind: "frame", frame: photoFrame, composedPhoto: composedSheet.frames[0].photo,
};
const portraitFrame: InspectorContext = {
  kind: "frame",
  frame: { ...photoFrame, id: "frame-002", photo: { ...photoFrame.photo!, mediaId: portrait.id } },
  composedPhoto: { ...composedSheet.frames[0].photo!, mediaId: portrait.id, name: portrait.name },
};

/**
 * The off-screen image the preview waits for before a swap. jsdom neither
 * loads nor decodes images, so each test settles the decode itself.
 */
class PreloadImage {
  static created: PreloadImage[] = [];
  crossOrigin: string | null = null;
  src = "";
  naturalWidth = 0;
  naturalHeight = 0;
  private settle: { resolve(): void; reject(error: Error): void } | null = null;
  private readonly decoding = new Promise<void>((resolve, reject) => {
    this.settle = { resolve, reject };
  });

  constructor() {
    PreloadImage.created.push(this);
  }

  decode() {
    return this.decoding;
  }

  finish(width: number, height: number) {
    this.naturalWidth = width;
    this.naturalHeight = height;
    this.settle!.resolve();
  }

  fail() {
    this.settle!.reject(new Error("EncodingError"));
  }
}

beforeEach(() => {
  PreloadImage.created = [];
  vi.stubGlobal("Image", PreloadImage);
});

afterEach(() => {
  vi.useRealTimers();
});

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

function previewBlock() {
  return document.querySelector<HTMLElement>(".inspector-image-preview");
}

function previewImages() {
  return Array.from(document.querySelectorAll<HTMLImageElement>(".inspector-image-preview img"));
}

function shownSources() {
  return previewImages().map((image) => image.getAttribute("src"));
}

function shownAspectRatio() {
  return document.querySelector<HTMLElement>(".inspector-image-preview .media-preview-thumbnail")!
    .style.getPropertyValue("--media-aspect-ratio");
}

function preloadOf(url: string) {
  const preload = [...PreloadImage.created].reverse().find((image) => image.src === url);
  expect(preload, `no preload of ${url}`).toBeDefined();
  return preload!;
}

async function finishDecode(url: string, width = 1_200, height = 800) {
  await act(async () => preloadOf(url).finish(width, height));
}

function loadPreview() {
  const [image] = previewImages();
  Object.defineProperty(image, "naturalWidth", { configurable: true, value: 1_200 });
  Object.defineProperty(image, "naturalHeight", { configurable: true, value: 800 });
  fireEvent.load(image);
}

test("a Frame with a Photo shows only its whole preview between the heading and Design", () => {
  renderInspector(landscapeFrame);

  const heading = screen.getByRole("heading", { name: "Serra ao amanhecer.jpg" });
  const preview = screen.getByRole("img", { name: "Prévia de Serra ao amanhecer.jpg" });
  const design = screen.getByRole("button", { name: "Design" });
  expect(heading.compareDocumentPosition(preview) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  expect(preview.compareDocumentPosition(design) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  // Fixed: no section header and nothing to collapse.
  expect(preview.closest("section")).toBeNull();
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
  expect(previewImages()[0]).toHaveAttribute("loading", "eager");
  expect(preview.querySelector(".media-preview-thumbnail")).toHaveStyle({ "--media-aspect-ratio": "6000 / 4000" });
  // The author left the pixel size out: the block holds the image alone.
  expect(previewBlock()!.textContent).toBe("");
});

test("the preview box is a fixed square as wide as the panel content, up to 360 px", () => {
  const styles = readFileSync("src/project/inspector/InspectorImagePreview.css", "utf8") as string;
  const rule = (selector: string) => {
    const match = styles.match(new RegExp(`(?:^|\\n)${selector.replace(/[.[\]"=>]/g, "\\$&")}[^{]*\\{([^}]*)\\}`));
    expect(match, `missing rule ${selector}`).not.toBeNull();
    return match![1].replace(/\s+/g, " ");
  };
  expect(rule(".inspector-image-preview")).toContain("--inspector-image-preview-max-side: 360px;");
  const box = rule(".inspector-image-preview__frame");
  expect(box).toContain("width: min(100%, var(--inspector-image-preview-max-side));");
  expect(box).toContain("aspect-ratio: 1 / 1;");
  // The image measures itself against the square's side.
  expect(box).toContain("container-type: inline-size;");
  expect(rule(".inspector-image-preview__frame > .media-preview-thumbnail"))
    .toContain("width: min(100cqi, calc(100cqi * (var(--media-aspect-ratio))));");
  // No fixed height is left to break the square.
  expect(styles).not.toMatch(/max-height:\s*200px|--inspector-image-preview-max-height/);
});

test("an empty Frame and several Frames have no preview", () => {
  const view = renderInspector({ kind: "frame", frame: { ...photoFrame, photo: null }, composedPhoto: null });
  expect(screen.getByRole("heading", { name: "Quadro vazio" })).toBeInTheDocument();
  expect(previewBlock()).toBeNull();

  view.rerenderInspector({
    kind: "multiple-frames",
    frames: [photoFrame, { ...photoFrame, id: "frame-002" }],
    editingSheet: composedSheet,
  });
  expect(screen.getByRole("heading", { name: "2 quadros selecionados" })).toBeInTheDocument();
  expect(screen.getByText("2 fotos · 0 quadros vazios")).toBeInTheDocument();
  expect(previewBlock()).toBeNull();
});

test("a selected image replaces the Album with its heading and preview", () => {
  renderInspector({ kind: "media", media: landscape });

  expect(screen.getByText("Imagem selecionada")).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "Serra ao amanhecer.jpg" })).toBeInTheDocument();
  expect(screen.getByRole("img", { name: "Prévia de Serra ao amanhecer.jpg" })).toBeInTheDocument();
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
  expect(previewBlock()!.textContent).toBe("");
  for (const name of ["Informações do álbum", "Design do álbum", "Grade de lâminas", "Design da lâmina", "Design"]) {
    expect(screen.queryByRole("button", { name })).not.toBeInTheDocument();
  }
});

test("a first preview shows at once, and a Photo without observed metadata takes the loaded proportion", () => {
  renderInspector({ kind: "media", media: { ...portrait, sourceWidthPx: 1, sourceHeightPx: 1 } }, {
    mediaPreviews: { [portrait.id]: { mediaId: portrait.id, state: "ready", url: PORTRAIT_URL } },
  });
  // Nothing to keep on screen: no waiting for a decode.
  expect(PreloadImage.created).toEqual([]);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
  expect(shownAspectRatio()).toBe("1 / 1");
  // The loaded preview, not the placeholder 1 × 1, gives the proportion.
  loadPreview();
  expect(shownAspectRatio()).toBe("1200 / 800");
});

test("a Decorative has a preview, and the next image brings its decoded proportion with it", async () => {
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
  expect(previewBlock()).toHaveAttribute("data-media-id", "decorative-001");
  expect(preloadOf(PORTRAIT_URL).crossOrigin).toBe("anonymous");

  await finishDecode(PORTRAIT_URL, 1_200, 800);
  // Image and proportion arrive in the same commit: no 1 × 1 box in between.
  expect(previewBlock()).toHaveAttribute("data-media-id", portrait.id);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
  expect(shownAspectRatio()).toBe("1200 / 800");
});

test("several selected images show only a heading", () => {
  renderInspector({ kind: "multiple-media", count: 3 });
  expect(screen.getByText("Seleção múltipla")).toBeInTheDocument();
  expect(screen.getByRole("heading", { name: "3 imagens selecionadas" })).toBeInTheDocument();
  expect(previewBlock()).toBeNull();
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
  // The same image: no wait.
  expect(PreloadImage.created).toEqual([]);
  expect(screen.getByRole("img", { name: "Prévia de Campo.jpg" })).toBeInTheDocument();
  expect(shownSources()).toEqual([PORTRAIT_URL]);
});

test("switching images keeps the previous image until the next one is decoded, then swaps image and proportion together", async () => {
  const view = renderInspector({ kind: "media", media: landscape }, ALL_READY);
  loadPreview();
  const previous = previewImages()[0];

  view.rerenderInspector({ kind: "media", media: portrait }, ALL_READY);
  expect(screen.getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  // Only while the next preview decodes: the photo, its box and its name stay.
  expect(previewBlock()).toHaveAttribute("data-media-id", landscape.id);
  expect(previewImages()).toEqual([previous]);
  expect(shownAspectRatio()).toBe("6000 / 4000");
  expect(screen.getByRole("img", { name: "Prévia de Serra ao amanhecer.jpg" })).toBeInTheDocument();
  // The same CORS mode as the visible <img>, so it reuses the decoded resource.
  expect(PreloadImage.created.map((image) => [image.src, image.crossOrigin])).toEqual([[PORTRAIT_URL, "anonymous"]]);

  await finishDecode(PORTRAIT_URL, 800, 1_200);
  expect(previewBlock()).toHaveAttribute("data-media-id", portrait.id);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
  expect(previewImages()[0]).not.toHaveAttribute("data-pending");
  expect(shownAspectRatio()).toBe("4000 / 6000");
  expect(screen.getByRole("img", { name: "Prévia de Campo.jpg" })).toBeInTheDocument();
});

test("a slow preview replaces the previous image at the wait limit", async () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
  const view = renderInspector({ kind: "media", media: landscape }, ALL_READY);

  view.rerenderInspector({ kind: "media", media: portrait }, ALL_READY);
  act(() => vi.advanceTimersByTime(INSPECTOR_PREVIEW_SWAP_WAIT_MS - 1));
  expect(shownSources()).toEqual([LANDSCAPE_URL]);

  act(() => vi.advanceTimersByTime(1));
  // Not decoded yet: the thumbnail's own loading state, in the new box.
  expect(previewBlock()).toHaveAttribute("data-media-id", portrait.id);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
  expect(shownAspectRatio()).toBe("4000 / 6000");

  // A late decode changes nothing.
  await finishDecode(PORTRAIT_URL, 800, 1_200);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
});

test("a preview that fails to decode replaces the previous image at once", async () => {
  const view = renderInspector({ kind: "media", media: landscape }, ALL_READY);

  view.rerenderInspector({ kind: "media", media: portrait }, ALL_READY);
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
  await act(async () => preloadOf(PORTRAIT_URL).fail());
  expect(previewBlock()).toHaveAttribute("data-media-id", portrait.id);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
});

test("an image with no preview to show replaces the previous one at once, and the stripes or missing symbol wait for the next preview", async () => {
  const view = renderInspector({ kind: "media", media: landscape });
  const stripes = () => document.querySelector(".inspector-image-preview .media-preview-thumbnail");

  // Its preview is still being prepared: the loading stripes, at once.
  view.rerenderInspector({ kind: "media", media: beach });
  expect(PreloadImage.created).toEqual([]);
  expect(previewBlock()).toHaveAttribute("data-media-id", beach.id);
  expect(previewImages()).toEqual([]);
  expect(stripes()).toHaveAttribute("data-has-preview", "false");

  // From the stripes to a preview: the stripes stay until it is decoded, with
  // no empty white box before the photo.
  view.rerenderInspector({ kind: "media", media: landscape });
  expect(screen.getByRole("heading", { name: "Serra ao amanhecer.jpg" })).toBeInTheDocument();
  expect(previewBlock()).toHaveAttribute("data-media-id", beach.id);
  expect(stripes()).toHaveAttribute("data-has-preview", "false");
  await finishDecode(LANDSCAPE_URL);
  expect(shownSources()).toEqual([LANDSCAPE_URL]);

  // A missing file without a retained preview: the missing symbol, at once.
  view.rerenderInspector({ kind: "media", media: portrait }, { missingMediaIds: new Set([portrait.id]) });
  expect(PreloadImage.created.map((image) => image.src)).toEqual([LANDSCAPE_URL]);
  expect(screen.getByRole("img", { name: "Arquivo ausente" })).toBeInTheDocument();
  expect(previewImages()).toEqual([]);

  // And the symbol, like the stripes, stays until the next preview is decoded.
  view.rerenderInspector({ kind: "media", media: landscape });
  expect(screen.getByRole("img", { name: "Arquivo ausente" })).toBeInTheDocument();
  await finishDecode(LANDSCAPE_URL);
  expect(screen.getByRole("img", { name: "Prévia de Serra ao amanhecer.jpg" })).toBeInTheDocument();
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
});

test("a missing file with a retained preview waits for that preview like any other image", async () => {
  const view = renderInspector({ kind: "media", media: landscape }, ALL_READY);
  const retained = {
    mediaPreviews: {
      ...ALL_READY.mediaPreviews,
      [portrait.id]: { mediaId: portrait.id, state: "absent" as const, url: PORTRAIT_URL },
    },
    missingMediaIds: new Set([portrait.id]),
  };

  view.rerenderInspector({ kind: "media", media: portrait }, retained);
  expect(screen.getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  expect(previewBlock()).toHaveAttribute("data-media-id", landscape.id);
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
  expect(PreloadImage.created.map((image) => image.src)).toEqual([PORTRAIT_URL]);

  await finishDecode(PORTRAIT_URL, 800, 1_200);
  // The retained preview, silently: no missing symbol.
  expect(previewBlock()).toHaveAttribute("data-media-id", portrait.id);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
  expect(screen.getByRole("img", { name: "Prévia de Campo.jpg" })).toBeInTheDocument();
  expect(document.querySelector(".inspector-image-preview .media-preview-thumbnail"))
    .not.toHaveAttribute("data-missing", "true");
});

test("quick switches end on the last image and never show an earlier one after it", async () => {
  const view = renderInspector({ kind: "media", media: landscape }, ALL_READY);

  view.rerenderInspector({ kind: "media", media: beach }, ALL_READY);
  view.rerenderInspector({ kind: "media", media: portrait }, ALL_READY);
  expect(screen.getByRole("heading", { name: "Campo.jpg" })).toBeInTheDocument();
  // The image in between finishes first: it was already replaced, so it stays off screen.
  await finishDecode(BEACH_URL);
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
  await finishDecode(PORTRAIT_URL, 800, 1_200);
  expect(shownSources()).toEqual([PORTRAIT_URL]);

  // And when the last image is ready before the one in between.
  view.rerenderInspector({ kind: "media", media: landscape }, ALL_READY);
  view.rerenderInspector({ kind: "media", media: beach }, ALL_READY);
  await finishDecode(BEACH_URL);
  await finishDecode(LANDSCAPE_URL);
  expect(previewBlock()).toHaveAttribute("data-media-id", beach.id);
  expect(shownSources()).toEqual([BEACH_URL]);

  // Back to the shown image before the next one is ready: no wait at all.
  view.rerenderInspector({ kind: "media", media: portrait }, ALL_READY);
  view.rerenderInspector({ kind: "media", media: beach }, ALL_READY);
  expect(previewBlock()).toHaveAttribute("data-media-id", beach.id);
  await finishDecode(PORTRAIT_URL, 800, 1_200);
  expect(shownSources()).toEqual([BEACH_URL]);
});

test("each quick switch gets the whole wait, and only the last image appears at its limit", () => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "performance"] });
  const view = renderInspector({ kind: "media", media: landscape }, ALL_READY);

  view.rerenderInspector({ kind: "media", media: beach }, ALL_READY);
  act(() => vi.advanceTimersByTime(200));
  view.rerenderInspector({ kind: "media", media: portrait }, ALL_READY);
  act(() => vi.advanceTimersByTime(INSPECTOR_PREVIEW_SWAP_WAIT_MS - 1));
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
  act(() => vi.advanceTimersByTime(1));
  expect(previewBlock()).toHaveAttribute("data-media-id", portrait.id);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
});

test("the preview stays mounted between a Frame and a selected image", async () => {
  const view = renderInspector(landscapeFrame, ALL_READY);
  const block = previewBlock();
  const [image] = previewImages();

  // The same photo: the very same image element, nothing to load again.
  view.rerenderInspector({ kind: "media", media: landscape }, ALL_READY);
  expect(screen.getByText("Imagem selecionada")).toBeInTheDocument();
  expect(previewBlock()).toBe(block);
  expect(previewImages()).toEqual([image]);
  expect(PreloadImage.created).toEqual([]);

  // Another photo: the Frame's photo stays until the next one is decoded.
  view.rerenderInspector({ kind: "media", media: portrait }, ALL_READY);
  expect(previewBlock()).toBe(block);
  expect(previewImages()).toEqual([image]);
  await finishDecode(PORTRAIT_URL, 800, 1_200);
  expect(previewBlock()).toBe(block);
  expect(shownSources()).toEqual([PORTRAIT_URL]);

  // And back to another Frame.
  view.rerenderInspector(landscapeFrame, ALL_READY);
  expect(screen.getByText("Quadro selecionado")).toBeInTheDocument();
  expect(shownSources()).toEqual([PORTRAIT_URL]);
  await finishDecode(LANDSCAPE_URL);
  expect(previewBlock()).toBe(block);
  expect(shownSources()).toEqual([LANDSCAPE_URL]);

  // Frame to Frame.
  view.rerenderInspector(portraitFrame, ALL_READY);
  expect(shownSources()).toEqual([LANDSCAPE_URL]);
  await finishDecode(PORTRAIT_URL, 800, 1_200);
  expect(previewBlock()).toBe(block);
  expect(shownSources()).toEqual([PORTRAIT_URL]);
});

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
    landscapeFrame,
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
