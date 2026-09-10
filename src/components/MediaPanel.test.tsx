import { createRef } from "react";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type { MediaCatalogItem, MediaUsage } from "../domain/project";
import { MediaPanel, type MediaPanelHandle } from "./MediaPanel";

const mediaItems: readonly MediaCatalogItem[] = [
  media("photo-album-10", "photo", "Álbum 10"),
  media("photo-album-2", "photo", "album 2"),
  media("photo-retrato", "photo", "Retrato", 800, 1200),
  media("decorative-overlay", "decorative", "Overlay dourado"),
];

const mediaUsage: readonly MediaUsage[] = [
  { count: 2, mediaId: "photo-album-10" },
  { count: 0, mediaId: "photo-album-2" },
  { count: 1, mediaId: "photo-retrato" },
  { count: 0, mediaId: "decorative-overlay" },
];

const mediaPanelInteractions = {
  onImportPhoto: () => undefined,
  onPhotoDragStart: () => undefined,
  onPhotoDragEnd: () => undefined,
  onRelinkMedia: () => undefined,
  onRetryUnavailableMedia: async () => undefined,
};

test("requests the initial measured viewport without waiting for a scroll or observer paint", () => {
  vi.stubGlobal("IntersectionObserver", class {
    observe() {}
    disconnect() {}
  });
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(202);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(114);
  const onDemandChange = vi.fn();
  const view = render(<MediaPanel {...mediaPanelInteractions}
    mediaItems={mediaItems} mediaUsage={mediaUsage} onFillPhoto={() => undefined}
    preferences={{kind: "local"}}
    previewSource={{kind: "connected", previews: {}, onDemandChange}}
  />);
  try {
    expect(onDemandChange).toHaveBeenLastCalledWith(expect.objectContaining({
      visibleMediaIds: expect.arrayContaining(["photo-album-2"]),
    }));
    const grid = screen.getByRole("group", { name: "Grade de Fotos" });
    grid.scrollTop = 190;
    fireEvent.scroll(grid);
    expect(onDemandChange).toHaveBeenLastCalledWith(expect.objectContaining({
      visibleMediaIds: expect.arrayContaining(["photo-retrato"]),
    }));
  } finally {
    view.unmount();
    width.mockRestore();
    height.mockRestore();
    vi.unstubAllGlobals();
  }
});

test("preloads upcoming viewports before the user scrolls, within a bounded window", () => {
  const width = vi.spyOn(Element.prototype, "clientWidth", "get").mockReturnValue(84);
  const height = vi.spyOn(Element.prototype, "clientHeight", "get").mockReturnValue(168);
  const onDemandChange = vi.fn();
  const view = render(<MediaPanel {...mediaPanelInteractions}
    mediaItems={Array.from({length: 100}, (_, i) => media(`photo-${i}`, "photo", `Foto ${i}`))}
    mediaUsage={[]} onFillPhoto={() => undefined} preferences={{kind: "local"}}
    previewSource={{kind: "connected", previews: {}, onDemandChange}}
  />);
  try {
    expect(onDemandChange).toHaveBeenLastCalledWith({
      visibleMediaIds: ["photo-0", "photo-1"],
      preloadMediaIds: ["photo-2", "photo-3", "photo-4", "photo-5", "photo-6", "photo-7"],
    });
  } finally {
    view.unmount();
    width.mockRestore();
    height.mockRestore();
  }
});

test("prepares only the future viewport with the panel's active ordering and filters", () => {
  const ref = createRef<MediaPanelHandle>();
  const demand = vi.fn();
  const photos = Array.from({ length: 100 }, (_, i) => media(`photo-${i}`, "photo", `Álbum ${i}`));
  const props = {
    ...mediaPanelInteractions, mediaUsage: [], onFillPhoto: vi.fn(),
    previewSource: { kind: "connected" as const, onDemandChange: demand, previews: {} },
    preferences: { kind: "local" as const, initial: { photo: {
      thumbnailSize: 84, sortDirection: "descending" as const, usageFilter: "all" as const,
    } } },
  };
  const view = render(<MediaPanel {...props} ref={ref} mediaItems={mediaItems} />);
  const grid = screen.getByRole("group", { name: "Grade de Fotos" });
  Object.defineProperties(grid, { clientWidth: { value: 202 }, clientHeight: { value: 114 } });
  Object.assign(grid.style, { padding: "10px 12px", rowGap: "10px", columnGap: "10px" });
  grid.scrollTop = 188;
  const plan = ref.current!.planCatalog(photos, []);
  expect(plan.demand.visibleMediaIds).toEqual(["photo-95", "photo-94", "photo-93", "photo-92"]);
  expect(plan.demand.preloadMediaIds).not.toContain("photo-80");
  plan.commit();
  view.rerender(<MediaPanel {...props} ref={ref} mediaItems={photos} />);
  expect(demand).toHaveBeenLastCalledWith(plan.demand);
  fireEvent.change(screen.getByRole("searchbox", { name: "Buscar Fotos" }), { target: { value: "album 99" } });
  // A shorter filtered catalog clamps scroll just as the browser does.
  expect(ref.current!.planCatalog(photos, []).demand).toEqual({
    visibleMediaIds: ["photo-99"], preloadMediaIds: [],
  });
  fireEvent.change(screen.getByRole("searchbox", { name: "Buscar Fotos" }), { target: { value: "nenhuma" } });
  expect(ref.current!.planCatalog(photos, []).demand).toEqual({ visibleMediaIds: [], preloadMediaIds: [] });
});

test("matches the reference toolbar and marks only unavailable import actions as placeholders", async () => {
  const user = userEvent.setup();
  renderPanel();

  expect(screen.getByRole("button", { name: "Fotos" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(screen.getByRole("button", { name: "Decorativos" })).toHaveAttribute(
    "aria-pressed",
    "false",
  );
  const photoSearch = screen.getByRole("searchbox", { name: "Buscar Fotos" });
  expect(photoSearch).toBeVisible();
  expect(photoSearch).toHaveClass("ui-embedded-input");
  expect(photoSearch.closest(".media-search")).toHaveClass(
    "ui-embedded-field",
  );
  expect(screen.getByRole("button", { name: "Todas 3" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(
    screen.getByRole("button", { name: "Nova pasta de organização" }),
  ).toHaveAttribute("data-placeholder-feature", "media-organization-folders");
  expect(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  ).toBeVisible();
  expect(
    screen.queryByRole("combobox", { name: "Filtro de uso" }),
  ).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Importar" }));
  const importMenu = screen.getByRole("menu", { name: "Importar" });
  expect(
    within(importMenu).getByRole("menuitem", { name: "Arquivos JPEG…" }),
  ).toBeEnabled();
  const folderItem = within(importMenu).getByRole("menuitem", {
    name: "Pasta…",
  });
  expect(folderItem).toBeDisabled();
  expect(folderItem).toHaveAttribute(
    "data-placeholder-feature",
    "import-media-folder",
  );
});

test("renders only centered copy when the media catalog is empty", () => {
  render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={[]}
      mediaUsage={[]}
      onFillPhoto={vi.fn()}
      previewSource={{ kind: "static" }}
      preferences={{ kind: "local" }}
    />,
  );

  const emptyState = screen.getByRole("status", {
    name: "Nenhuma Foto importada",
  });
  expect(emptyState).toHaveClass("ui-empty-state");
  expect(emptyState).toHaveClass("media-empty-state--catalog");
  expect(screen.getByRole("group", { name: "Grade de Fotos" })).toHaveAttribute(
    "data-empty",
    "catalog",
  );
  expect(emptyState).toHaveTextContent(
    "As Fotos importadas para este Projeto aparecerão aqui.",
  );
  expect(emptyState.querySelector(".ui-empty-state__eyebrow")).toBeNull();
  expect(emptyState.querySelector(".ui-empty-state__icon")).toBeNull();
});

test("keeps only centered copy when filters have no results", async () => {
  const user = userEvent.setup();
  renderPanel();

  await user.type(
    screen.getByRole("searchbox", { name: "Buscar Fotos" }),
    "inexistente",
  );

  const emptyState = screen.getByRole("status", {
    name: "Nenhum item encontrado",
  });
  expect(emptyState).toHaveClass("media-empty-state--filtered");
  expect(screen.getByRole("group", { name: "Grade de Fotos" })).toHaveAttribute(
    "data-empty",
    "filtered",
  );
  expect(emptyState).toHaveTextContent(
    "Ajuste a Busca ou o Filtro de uso para ver outros itens.",
  );
  expect(emptyState.querySelector(".ui-empty-state__eyebrow")).toBeNull();
  expect(emptyState.querySelector(".ui-empty-state__icon")).toBeNull();
});

test("combines accent-insensitive search with the usage filter and natural name order", async () => {
  const user = userEvent.setup();
  renderPanel();

  await user.type(screen.getByRole("searchbox", { name: "Buscar Fotos" }), "album");
  expect(visibleMediaIds()).toEqual(["photo-album-2", "photo-album-10"]);

  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Ordenar por" }),
    "name-descending",
  );
  expect(visibleMediaIds()).toEqual(["photo-album-10", "photo-album-2"]);
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Ordenar por" }),
    "name-ascending",
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Filtro de uso" }),
    "used",
  );
  expect(visibleMediaIds()).toEqual(["photo-album-10"]);
  expect(screen.getByRole("combobox", { name: "Filtro de uso" })).toHaveValue(
    "used",
  );
});

test("treats compact options as a disclosure and restores its trigger on Escape", async () => {
  const user = userEvent.setup();
  renderPanel();

  const trigger = screen.getByRole("button", {
    name: "Filtro, ordem e tamanho",
  });
  await user.click(trigger);
  const usageFilter = screen.getByRole("combobox", { name: "Filtro de uso" });
  await user.click(usageFilter);
  expect(usageFilter).toHaveFocus();

  await user.keyboard("{Escape}");

  expect(trigger).toHaveFocus();
  expect(
    screen.queryByRole("combobox", { name: "Filtro de uso" }),
  ).not.toBeInTheDocument();
});

test("keeps independent search text for Fotos and Decorativos", async () => {
  const user = userEvent.setup();
  renderPanel();

  const photoSearch = screen.getByRole("searchbox", { name: "Buscar Fotos" });
  await user.type(photoSearch, "retrato");
  await user.click(screen.getByRole("button", { name: "Decorativos" }));

  const decorativeSearch = screen.getByRole("searchbox", {
    name: "Buscar Decorativos",
  });
  expect(decorativeSearch).toHaveValue("");
  await user.type(decorativeSearch, "dourado");

  await user.click(screen.getByRole("button", { name: "Fotos" }));
  expect(screen.getByRole("searchbox", { name: "Buscar Fotos" })).toHaveValue(
    "retrato",
  );
});

test("resizes thumbnails independently per tab and marks unavailable date ordering in code", async () => {
  const user = userEvent.setup();
  renderPanel();

  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  const size = screen.getByRole("slider", { name: "Tamanho das miniaturas" });
  fireEvent.change(size, { target: { value: "124" } });

  expect(screen.getByRole("group", { name: "Grade de Fotos" })).toHaveStyle({
    "--media-thumbnail-size": "124px",
  });

  const dateOption = screen.getByRole("option", { name: "Data de criação" });
  expect(dateOption).toBeDisabled();
  expect(dateOption).toHaveAttribute(
    "data-placeholder-feature",
    "sort-media-by-created-at",
  );

  await user.click(screen.getByRole("button", { name: "Decorativos" }));
  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  const decorativeSize = screen.getByRole("slider", {
    name: "Tamanho das miniaturas",
  });
  expect(decorativeSize).toHaveValue("84");
  fireEvent.change(decorativeSize, { target: { value: "110" } });

  await user.click(screen.getByRole("button", { name: "Fotos" }));
  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  const restoredPhotoSize = screen.getByRole("slider", {
    name: "Tamanho das miniaturas",
  });
  expect(restoredPhotoSize).toHaveValue("124");
  fireEvent.doubleClick(restoredPhotoSize);
  expect(restoredPhotoSize).toHaveValue("84");
});

test("hydrates per-tab thumbnail sizes and publishes later changes", async () => {
  const user = userEvent.setup();
  const onThumbnailSizeChange = vi.fn();
  render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={mediaItems}
      mediaUsage={mediaUsage}
      onFillPhoto={vi.fn()}
      previewSource={{ kind: "static" }}
      preferences={{
        kind: "controlled",
        persistent: {
          decorative: { sortDirection: "ascending", usageFilter: "all" },
          photo: { sortDirection: "ascending", usageFilter: "all" },
        },
        thumbnailSizes: { decorative: 110, photo: 124 },
        onSortDirectionChange: vi.fn(),
        onThumbnailSizeChange,
        onUsageFilterChange: vi.fn(),
      }}
    />,
  );

  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  const photoSize = screen.getByRole("slider", {
    name: "Tamanho das miniaturas",
  });
  expect(photoSize).toHaveValue("124");
  fireEvent.change(photoSize, { target: { value: "126" } });
  expect(onThumbnailSizeChange).toHaveBeenCalledWith("photo", 126);

  await user.click(screen.getByRole("button", { name: "Decorativos" }));
  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  expect(
    screen.getByRole("slider", { name: "Tamanho das miniaturas" }),
  ).toHaveValue("110");
});

test("hydrates authoritative per-tab settings and publishes only the changed field", async () => {
  const user = userEvent.setup();
  const onSortDirectionChange = vi.fn();
  const onUsageFilterChange = vi.fn();
  render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={mediaItems}
      mediaUsage={mediaUsage}
      onFillPhoto={vi.fn()}
      previewSource={{ kind: "static" }}
      preferences={{
        kind: "controlled",
        persistent: {
          decorative: { sortDirection: "ascending", usageFilter: "all" },
          photo: { sortDirection: "descending", usageFilter: "unused" },
        },
        thumbnailSizes: { decorative: 84, photo: 84 },
        onSortDirectionChange,
        onThumbnailSizeChange: vi.fn(),
        onUsageFilterChange,
      }}
    />,
  );

  expect(visibleMediaIds()).toEqual(["photo-album-2"]);
  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  expect(screen.getByRole("combobox", { name: "Ordenar por" })).toHaveValue(
    "name-descending",
  );
  expect(screen.getByRole("combobox", { name: "Filtro de uso" })).toHaveValue(
    "unused",
  );

  await user.selectOptions(
    screen.getByRole("combobox", { name: "Filtro de uso" }),
    "used",
  );
  expect(onUsageFilterChange).toHaveBeenCalledWith("photo", "used");
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Ordenar por" }),
    "name-ascending",
  );
  expect(onSortDirectionChange).toHaveBeenCalledWith("photo", "ascending");

  await user.click(screen.getByRole("button", { name: "Decorativos" }));
  await user.click(
    screen.getByRole("button", { name: "Filtro, ordem e tamanho" }),
  );
  expect(screen.getByRole("combobox", { name: "Ordenar por" })).toHaveValue(
    "name-ascending",
  );
  expect(screen.getByRole("combobox", { name: "Filtro de uso" })).toHaveValue(
    "all",
  );
});

test("keeps the last observed viewport warm across Fotos and Decorativos", () => {
  vi.stubGlobal("IntersectionObserver", class {
    constructor(private callback: IntersectionObserverCallback) {}
    observe(target: HTMLElement) {
      this.callback([{ target, isIntersecting: target.dataset.mediaId !== "photo-retrato" } as unknown as IntersectionObserverEntry], this as unknown as IntersectionObserver);
    }
    disconnect() {}
  });
  const onDemandChange = vi.fn();
  const view = render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={mediaItems}
      mediaUsage={mediaUsage}
      onFillPhoto={vi.fn()}
      previewSource={{ kind: "connected", previews: {}, onDemandChange }}
      preferences={{ kind: "local" }}
    />,
  );
  try {
    onDemandChange.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
    expect(onDemandChange).toHaveBeenLastCalledWith({
      visibleMediaIds: ["decorative-overlay"],
      preloadMediaIds: ["photo-album-2", "photo-album-10"],
    });
    expect(onDemandChange.mock.calls.every(([demand]) =>
      [...demand.visibleMediaIds, ...demand.preloadMediaIds].includes("photo-album-2"),
    )).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "Fotos" }));
    expect(onDemandChange).toHaveBeenLastCalledWith({
      visibleMediaIds: ["photo-album-2", "photo-album-10"],
      preloadMediaIds: ["decorative-overlay"],
    });
  } finally {
    view.unmount();
    vi.unstubAllGlobals();
  }
});

test("retires scrolled and removed media and ignores disconnected observers", () => {
  const observers: { callback: IntersectionObserverCallback; targets: HTMLElement[] }[] = [];
  vi.stubGlobal("IntersectionObserver", class {
    targets: HTMLElement[] = [];
    constructor(public callback: IntersectionObserverCallback) { observers.push(this); }
    observe(target: HTMLElement) { this.targets.push(target); }
    disconnect() {}
  });
  const onDemandChange = vi.fn();
  const props = {
    ...mediaPanelInteractions,
    mediaItems,
    mediaUsage,
    onFillPhoto: vi.fn(),
    previewSource: { kind: "connected" as const, previews: {}, onDemandChange },
    preferences: { kind: "local" as const },
  };
  const view = render(<MediaPanel {...props} />);
  const observeOnly = (index: number, mediaId: string) => {
    const observer = observers[index];
    observer.callback(observer.targets.map((target) => ({
      target, isIntersecting: target.dataset.mediaId === mediaId,
    }) as unknown as IntersectionObserverEntry), observer as unknown as IntersectionObserver);
  };
  try {
    observeOnly(0, "photo-album-2");
    observeOnly(1, "photo-album-2");
    observeOnly(0, "photo-retrato");
    observeOnly(1, "photo-retrato");
    expect(onDemandChange).toHaveBeenLastCalledWith({ visibleMediaIds: ["photo-retrato"], preloadMediaIds: [] });
    fireEvent.click(screen.getByRole("button", { name: "Decorativos" }));
    observeOnly(2, "decorative-overlay");
    observeOnly(3, "decorative-overlay");
    expect(onDemandChange).toHaveBeenLastCalledWith({ visibleMediaIds: ["decorative-overlay"], preloadMediaIds: ["photo-retrato"] });
    onDemandChange.mockClear();
    observeOnly(0, "photo-album-2");
    expect(onDemandChange).not.toHaveBeenCalled();
    view.rerender(<MediaPanel {...props} mediaItems={mediaItems.filter(({ id }) => id !== "photo-retrato")} />);
    expect(onDemandChange).toHaveBeenLastCalledWith({ visibleMediaIds: ["decorative-overlay"], preloadMediaIds: [] });
  } finally {
    view.unmount();
    vi.unstubAllGlobals();
  }
});

test("clears preview demand when the panel unmounts", () => {
  const onMediaDemandChange = vi.fn();
  const view = render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={mediaItems}
      mediaUsage={mediaUsage}
      onFillPhoto={vi.fn()}
      previewSource={{
        kind: "connected",
        previews: {},
        onDemandChange: onMediaDemandChange,
      }}
      preferences={{ kind: "local" }}
    />,
  );
  onMediaDemandChange.mockClear();

  view.unmount();

  expect(onMediaDemandChange).toHaveBeenCalledOnce();
  expect(onMediaDemandChange).toHaveBeenCalledWith({
    visibleMediaIds: [],
    preloadMediaIds: [],
  });
});

test("uses image orientation and opacity without visible names or usage counts", () => {
  render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={mediaItems}
      previewSource={{
        kind: "static",
        previews: {
          "photo-album-10": {
            mediaId: "photo-album-10",
            state: "ready",
            url: "/album-10.jpg",
          },
          "photo-retrato": {
            mediaId: "photo-retrato",
            state: "ready",
            url: "/retrato.jpg",
          },
        },
      }}
      mediaUsage={mediaUsage}
      onFillPhoto={vi.fn()}
      preferences={{ kind: "local" }}
    />,
  );

  const usedCard = document.querySelector<HTMLElement>(
    '[data-media-id="photo-album-10"]',
  );
  const portraitCard = document.querySelector<HTMLElement>(
    '[data-media-id="photo-retrato"]',
  );

  expect(usedCard).toHaveAttribute("data-used", "true");
  expect(usedCard).toHaveAccessibleName("Álbum 10. Já usada");
  expect(usedCard).not.toHaveTextContent("Álbum 10");
  expect(usedCard?.querySelector(".media-usage-badge")).toBeNull();
  expect(usedCard?.querySelector(".media-meta")).toBeNull();
  const landscapeThumb = usedCard?.querySelector<HTMLElement>(
    ".media-preview-thumbnail",
  );
  const portraitThumb =
    portraitCard?.querySelector<HTMLElement>(".media-preview-thumbnail");

  expect(landscapeThumb).toHaveAttribute("data-portrait", "false");
  expect(landscapeThumb).toHaveAttribute("data-has-preview", "true");
  expect(landscapeThumb).toHaveStyle({
    "--media-aspect-ratio": "1200 / 800",
  });
  expect(portraitThumb).toHaveAttribute("data-portrait", "true");
  expect(portraitThumb).toHaveStyle({
    "--media-aspect-ratio": "800 / 1200",
  });
});

test("keeps selection on media ids and supports click, Ctrl, Shift, and Ctrl+A", () => {
  renderPanel();

  const album2 = screen.getByRole("button", { name: "album 2" });
  const album10 = screen.getByRole("button", {
    name: "Álbum 10. Já usada",
  });
  const portrait = screen.getByRole("button", {
    name: "Retrato. Já usada",
  });
  const grid = screen.getByRole("group", { name: "Grade de Fotos" });

  expect(album2).toHaveAttribute("aria-pressed", "false");
  fireEvent.click(album2);
  expect(album2).toHaveAttribute("aria-pressed", "true");
  expect(album10).toHaveAttribute("aria-pressed", "false");

  fireEvent.click(portrait, { ctrlKey: true });
  expect(album2).toHaveAttribute("aria-pressed", "true");
  expect(portrait).toHaveAttribute("aria-pressed", "true");

  fireEvent.click(album10, { shiftKey: true });
  expect(album2).toHaveAttribute("aria-pressed", "true");
  expect(album10).toHaveAttribute("aria-pressed", "true");
  expect(portrait).toHaveAttribute("aria-pressed", "false");

  fireEvent.keyDown(grid, { ctrlKey: true, key: "a" });
  expect(album2).toHaveAttribute("aria-pressed", "true");
  expect(album10).toHaveAttribute("aria-pressed", "true");
  expect(portrait).toHaveAttribute("aria-pressed", "true");
});

test("preserves a selected group on right click and replaces it for an unselected item", () => {
  renderPanel();

  const album2 = screen.getByRole("button", { name: "album 2" });
  const album10 = screen.getByRole("button", {
    name: "Álbum 10. Já usada",
  });
  const portrait = screen.getByRole("button", {
    name: "Retrato. Já usada",
  });

  fireEvent.click(album2);
  fireEvent.click(portrait, { ctrlKey: true });
  fireEvent.contextMenu(portrait);
  expect(album2).toHaveAttribute("aria-pressed", "true");
  expect(portrait).toHaveAttribute("aria-pressed", "true");

  fireEvent.contextMenu(album10);
  expect(album2).toHaveAttribute("aria-pressed", "false");
  expect(album10).toHaveAttribute("aria-pressed", "true");
  expect(portrait).toHaveAttribute("aria-pressed", "false");
});

test("clears selection and keeps Ctrl+A in the grid after a background click", () => {
  renderPanel();

  const grid = screen.getByRole("group", { name: "Grade de Fotos" });
  const album2 = screen.getByRole("button", { name: "album 2" });
  fireEvent.click(album2);
  expect(album2).toHaveAttribute("aria-pressed", "true");

  fireEvent.click(grid);
  expect(album2).toHaveAttribute("aria-pressed", "false");
  expect(grid).toHaveFocus();

  expect(fireEvent.keyDown(grid, { ctrlKey: true, key: "a" })).toBe(false);
  const selectedMediaCards = within(grid).getAllByRole("button", {
    pressed: true,
  });
  for (const mediaCard of selectedMediaCards) {
    expect(mediaCard).toHaveAttribute("data-selected", "true");
  }
  expect(selectedMediaCards).toHaveLength(3);
});

test("leaves Ctrl+A to nested editable content inside the image panel", () => {
  renderPanel();

  const panel = screen.getByRole("region", { name: "Painel de imagens" });
  const editor = document.createElement("div");
  const text = document.createElement("span");
  editor.setAttribute("contenteditable", "");
  editor.append(text);
  panel.append(editor);

  expect(fireEvent.keyDown(text, { ctrlKey: true, key: "a" })).toBe(true);
  const cards = panel.querySelectorAll("button[data-media-id]");
  expect(cards).toHaveLength(3);
  for (const card of cards) {
    expect(card).toHaveAttribute("aria-pressed", "false");
  }
});

test("removes hidden items from the transient media selection", async () => {
  const user = userEvent.setup();
  renderPanel();

  await user.click(screen.getByRole("button", { name: "album 2" }));
  const search = screen.getByRole("searchbox", { name: "Buscar Fotos" });
  await user.type(search, "retrato");
  await user.clear(search);

  expect(screen.getByRole("button", { name: "album 2" })).toHaveAttribute(
    "aria-pressed",
    "false",
  );
});

test("uses the intrinsic preview ratio when source dimensions are unavailable", () => {
  const mediaWithoutDimensions = media(
    "photo-no-metadata",
    "photo",
    "Sem metadados",
    null,
    null,
  );
  render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={[mediaWithoutDimensions]}
      previewSource={{
        kind: "static",
        previews: {
          "photo-no-metadata": {
            mediaId: "photo-no-metadata",
            state: "ready",
            url: "/portrait-without-metadata.jpg",
          },
        },
      }}
      mediaUsage={[]}
      onFillPhoto={vi.fn()}
      preferences={{ kind: "local" }}
    />,
  );

  const image = document.querySelector<HTMLImageElement>(
    '[data-media-id="photo-no-metadata"] img',
  );
  expect(image).not.toBeNull();
  Object.defineProperties(image!, {
    naturalHeight: { configurable: true, value: 1200 },
    naturalWidth: { configurable: true, value: 800 },
  });
  fireEvent.load(image!);

  const thumb = document.querySelector<HTMLElement>(
    '[data-media-id="photo-no-metadata"] .media-preview-thumbnail',
  );
  expect(thumb).toHaveAttribute("data-portrait", "true");
  expect(thumb).toHaveStyle({
    "--media-aspect-ratio": "800 / 1200",
  });
});

function renderPanel() {
  return render(
    <MediaPanel
      {...mediaPanelInteractions}
      mediaItems={mediaItems}
      mediaUsage={mediaUsage}
      onFillPhoto={vi.fn()}
      previewSource={{ kind: "static" }}
      preferences={{ kind: "local" }}
    />,
  );
}

function media(
  id: string,
  kind: MediaCatalogItem["kind"],
  name: string,
  sourceWidthPx: number | null = 1200,
  sourceHeightPx: number | null = 800,
) {
  return {
    id,
    kind,
    name,
    palette: null,
    sourceHeightPx,
    sourceWidthPx,
  } satisfies MediaCatalogItem;
}

function visibleMediaIds() {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-media-id]")).map(
    (element) => element.dataset.mediaId,
  );
}
