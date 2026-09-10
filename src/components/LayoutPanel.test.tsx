import { fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import { layoutPanelCorpus } from "../test/layoutPanelPreview";
import { LayoutPanel } from "./LayoutPanel";
import type { LayoutPanelController } from "./useLayoutPanel";

function panel(overrides: Partial<LayoutPanelController> = {}, caseName = "mixed", stage: "before" | "lockReady" | "filled" = "before") {
  const sample = layoutPanelCorpus.cases[caseName][stage]!;
  const sheet = sample.projection.composition.sheets[0];
  const prepared = sample.queries[sheet.sheetId];
  const controller: LayoutPanelController = {
    visible: true, sheetId: sheet.sheetId, composition: sample.projection.composition,
    committing: false, query: prepared.query, displayQuery: overrides.query ?? prepared.query, previews: prepared.previews, error: null,
    toggle: vi.fn(), close: vi.fn(), preview: vi.fn(), cancelPreview: vi.fn(),
    apply: vi.fn(async () => true),
    lock: vi.fn(async () => true), unlock: vi.fn(async () => true),
    positionCount: sheet.frames.length, configurePositions: vi.fn(),
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

test("filled Frames are represented by generic geometry without photo content or decoration", () => {
  panel({}, "mixed", "filled");
  const region = screen.getByRole("region", { name: "Painel de Layouts" });
  const thumbnail = within(region).getAllByRole("img")[0];
  expect(thumbnail).toHaveAccessibleName("Layout com 4 Frames");
  expect(thumbnail.querySelectorAll("[data-preview-frame-id]")).toHaveLength(4);
  expect(thumbnail.querySelector("image, [data-preview-frame-content-id], [data-preview-frame-border-id]")).toBeNull();
});
