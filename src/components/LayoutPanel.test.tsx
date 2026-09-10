import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { layoutPanelCorpus } from "../test/layoutPanelPreview";
import { LayoutPanel } from "./LayoutPanel";
import type { LayoutPanelController } from "./useLayoutPanel";
import type { LayoutCatalogController } from "./useLayoutCatalog";

function panel(overrides: Partial<LayoutPanelController> = {}, caseName = "mixed", stage: "before" | "lockReady" | "filled" = "before") {
  const sample = layoutPanelCorpus.cases[caseName][stage]!;
  const sheet = sample.projection.composition.sheets[0];
  const prepared = sample.queries[sheet.sheetId];
  const controller: LayoutPanelController = {
    visible: true, sheetId: sheet.sheetId, composition: sample.projection.composition,
    committing: false, query: prepared.query, displayQuery: overrides.query ?? prepared.query, previews: prepared.previews, error: null,
    toggle: vi.fn(), close: vi.fn(), refresh: vi.fn(), preview: vi.fn(), cancelPreview: vi.fn(),
    apply: vi.fn(async () => true), toggleFavorite: vi.fn(async () => true),
    lock: vi.fn(async () => true), unlock: vi.fn(async () => true),
    positionCount: sheet.frames.length, minimumPositionCount: sheet.frames.filter((frame) => frame.photo !== null).length,
    configurePositions: vi.fn(),
    ...overrides,
  };
  const view = render(<LayoutPanel controller={controller} sheet={sheet} />);
  return { controller, view };
}

test("the lock remains actionable when extra positions disable the preview body", () => {
  const { controller } = panel({ positionCount: 6 }, "expanded", "lockReady");
  expect(screen.getByRole("button", { name: /^Aplicar Layout 1$/ })).toBeDisabled();
  const lock = screen.getByRole("button", { name: "Aplicar e travar Layout 1" });
  expect(lock).toBeEnabled();
  fireEvent.click(lock);
  expect(controller.lock).toHaveBeenCalledExactlyOnceWith(0);
  expect(controller.apply).not.toHaveBeenCalled();
});

test("the highlighted closed lock unlocks directly while other candidates stay disabled", () => {
  const prepared = structuredClone(layoutPanelCorpus.cases.mixed.before.queries["sheet-001"].query);
  prepared.locked = true;
  prepared.listing.candidates[0].isLastApplied = true;
  const { controller } = panel({ query: prepared });
  for (const candidate of screen.getAllByRole("button", { name: /^Aplicar/ })) expect(candidate).toBeDisabled();
  const unlock = screen.getByRole("button", { name: "Destravar Layout da Lâmina 01" });
  expect(unlock).toBeEnabled();
  fireEvent.click(unlock);
  expect(controller.unlock).toHaveBeenCalledOnce();
  expect(controller.lock).not.toHaveBeenCalled();
});

test("an outside press closes the panel even when the outside control stops propagation", () => {
  const { controller } = panel();
  render(<button onPointerDown={(event) => event.stopPropagation()}>Fora do painel</button>);
  fireEvent.pointerDown(screen.getByRole("combobox", { name: "Quantidade de Frames" }));
  expect(controller.close).not.toHaveBeenCalled();
  fireEvent.pointerDown(screen.getByRole("button", { name: "Fora do painel" }));
  expect(controller.close).toHaveBeenCalledOnce();
});

test("the frame count requests additional positions without applying a Layout", () => {
  const { controller } = panel();
  const count = screen.getByRole("combobox", { name: "Quantidade de Frames" });
  fireEvent.change(count, { target: { value: "6" } });
  expect(controller.configurePositions).toHaveBeenCalledExactlyOnceWith(6);
  expect(controller.apply).not.toHaveBeenCalled();
  expect(controller.lock).not.toHaveBeenCalled();
});

test("placeholders do not prevent choosing a smaller Layout that keeps every Photo", () => {
  const sample = layoutPanelCorpus.cases.mixed.before;
  const sheet = sample.projection.composition.sheets[0];
  const photoCount = sheet.frames.filter((frame) => frame.photo !== null).length;
  expect(photoCount).toBeGreaterThan(0);
  expect(photoCount).toBeLessThan(sheet.frames.length);
  const { controller } = panel();
  const count = screen.getByRole("combobox", { name: "Quantidade de Frames" });
  expect(within(count).getByRole("option", { name: String(photoCount) })).toBeInTheDocument();
  fireEvent.change(count, { target: { value: String(photoCount) } });
  expect(controller.configurePositions).toHaveBeenCalledExactlyOnceWith(photoCount);
  expect(controller.apply).not.toHaveBeenCalled();
});

test("filled Frames are represented by generic geometry without photo content or decoration", () => {
  panel({}, "mixed", "filled");
  const region = screen.getByRole("region", { name: "Painel de Layouts" });
  const thumbnail = within(region).getAllByRole("img")[0];
  expect(thumbnail).toHaveAccessibleName("Layout com 4 Frames");
  expect(thumbnail.querySelectorAll("[data-preview-frame-id]")).toHaveLength(4);
  expect(thumbnail.querySelector("image, [data-preview-frame-content-id], [data-preview-frame-border-id]")).toBeNull();
});

test("a pending custom duplicate is consumed only by a compatible, current query and reveals its existing card", () => {
  const sample = layoutPanelCorpus.cases.custom.before;
  const sheet = sample.projection.composition.sheets[0];
  const prepared = sample.queries[sheet.sheetId];
  const { controller, view } = panel({ query: null, displayQuery: null });
  const revealId = layoutPanelCorpus.cases.custom.saveResult!.layoutId;
  const catalog: LayoutCatalogController = { revision: 1, busy: false, notice: null, revealId,
    save: vi.fn(async () => undefined), refresh: vi.fn(async () => true), requestDelete: vi.fn(),
    acknowledgeReveal: vi.fn(), dismissNotice: vi.fn() };
  view.rerender(<LayoutPanel controller={controller} sheet={sheet} catalog={catalog} />);
  expect(catalog.acknowledgeReveal).not.toHaveBeenCalled();
  view.rerender(<LayoutPanel controller={{ ...controller, query: prepared.query, displayQuery: prepared.query, previews: prepared.previews }} sheet={sheet} catalog={catalog} />);
  expect(catalog.acknowledgeReveal).toHaveBeenCalledOnce();
  expect(document.querySelector(`[data-custom-layout-id="${revealId}"]`)).toHaveClass("layout-panel__candidate--revealed");
  fireEvent.click(screen.getByRole("button", { name: "Excluir Layout personalizado 1" }));
  expect(catalog.requestDelete).toHaveBeenCalledExactlyOnceWith(revealId);
});


test("stars keep their origin, expose their state and toggle independently of applying a Layout", () => {
  const sample = layoutPanelCorpus.cases.favorites.favoriteStates!.both;
  const sheet = sample.projection.composition.sheets[0];
  const prepared = sample.queries[sheet.sheetId];
  const { controller } = panel({ query: prepared.query, displayQuery: prepared.query, previews: prepared.previews });
  const favorites = screen.getAllByRole("button", { name: /^Remover dos favoritos Layout/ });
  expect(favorites).toHaveLength(2);
  for (const button of favorites) {
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(button).toBeEnabled();
  }
  expect(favorites.map((button) => button.closest(".layout-panel__section")?.querySelector("h3")?.textContent)).toEqual(["Automáticos", "Personalizados"]);
  fireEvent.click(favorites[1]);
  expect(controller.toggleFavorite).toHaveBeenCalledExactlyOnceWith(1);
  expect(controller.apply).not.toHaveBeenCalled();
  expect(controller.lock).not.toHaveBeenCalled();
});

test("the current locked Layout can be starred while stars on other candidates are disabled", () => {
  const query = structuredClone(layoutPanelCorpus.cases.mixed.locked!.queries["sheet-001"].query);
  panel({ query });
  const stars = screen.getAllByRole("button", { name: /^Favoritar Layout/ });
  expect(stars[0]).toBeEnabled();
  for (const star of stars.slice(1)) expect(star).toBeDisabled();
});
