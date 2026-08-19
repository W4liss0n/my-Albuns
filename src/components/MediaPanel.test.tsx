import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type { MediaCatalogItem, MediaUsage } from "../domain/project";
import { MediaPanel } from "./MediaPanel";

const mediaItems: readonly MediaCatalogItem[] = [
  media("photo-album-10", "photo", "Álbum 10"),
  media("photo-album-2", "photo", "album 2"),
  media("photo-retrato", "photo", "Retrato"),
  media("decorative-overlay", "decorative", "Overlay dourado"),
];

const mediaUsage: readonly MediaUsage[] = [
  { count: 2, mediaId: "photo-album-10" },
  { count: 0, mediaId: "photo-album-2" },
  { count: 1, mediaId: "photo-retrato" },
  { count: 0, mediaId: "decorative-overlay" },
];

test("matches the reference toolbar and exposes unavailable import actions as placeholders", async () => {
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
  expect(screen.getByRole("searchbox", { name: "Buscar Fotos" })).toBeVisible();
  expect(screen.getByRole("combobox", { name: "Filtro de uso" })).toBeVisible();
  expect(screen.getByRole("combobox", { name: "Ordenar por" })).toBeVisible();
  expect(
    screen.getByRole("slider", { name: "Tamanho das miniaturas" }),
  ).toBeVisible();

  await user.click(screen.getByRole("button", { name: "Importar" }));
  const importMenu = screen.getByRole("menu", { name: "Importar" });
  for (const [name, feature] of [
    ["Arquivos…", "import-media-files"],
    ["Pasta…", "import-media-folder"],
  ] as const) {
    const item = within(importMenu).getByRole("menuitem", { name });
    expect(item).toBeDisabled();
    expect(item).toHaveAttribute("data-placeholder-feature", feature);
  }
});

test("combines accent-insensitive search with the usage filter and natural name order", async () => {
  const user = userEvent.setup();
  renderPanel();

  await user.type(screen.getByRole("searchbox", { name: "Buscar Fotos" }), "album");
  expect(visibleMediaIds()).toEqual(["photo-album-2", "photo-album-10"]);

  await user.click(screen.getByRole("button", { name: "Ordem crescente" }));
  expect(visibleMediaIds()).toEqual(["photo-album-10", "photo-album-2"]);
  await user.click(screen.getByRole("button", { name: "Ordem decrescente" }));
  await user.selectOptions(screen.getByRole("combobox", { name: "Filtro de uso" }), "used");
  expect(visibleMediaIds()).toEqual(["photo-album-10"]);
  expect(screen.getByRole("combobox", { name: "Filtro de uso" })).toHaveValue(
    "used",
  );
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
  const decorativeSize = screen.getByRole("slider", {
    name: "Tamanho das miniaturas",
  });
  expect(decorativeSize).toHaveValue("90");
  fireEvent.change(decorativeSize, { target: { value: "110" } });

  await user.click(screen.getByRole("button", { name: "Fotos" }));
  const restoredPhotoSize = screen.getByRole("slider", {
    name: "Tamanho das miniaturas",
  });
  expect(restoredPhotoSize).toHaveValue("124");
  fireEvent.doubleClick(restoredPhotoSize);
  expect(restoredPhotoSize).toHaveValue("90");
});

function renderPanel() {
  return render(
    <MediaPanel
      mediaItems={mediaItems}
      mediaUsage={mediaUsage}
      onFillPhoto={vi.fn()}
    />,
  );
}

function media(id: string, kind: MediaCatalogItem["kind"], name: string) {
  return {
    id,
    kind,
    name,
    palette: null,
    sourceHeightPx: 800,
    sourceWidthPx: 1200,
  } satisfies MediaCatalogItem;
}

function visibleMediaIds() {
  return Array.from(document.querySelectorAll<HTMLElement>("[data-media-id]")).map(
    (element) => element.dataset.mediaId,
  );
}
