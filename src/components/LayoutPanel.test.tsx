import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { DisplayUnit } from "../domain/project";
import { layoutPanelCorpus } from "../test/layoutPanelPreview";
import { LayoutPanel } from "./LayoutPanel";
import type { LayoutPanelController } from "./useLayoutPanel";

function panel(unit: DisplayUnit, overrides: Partial<LayoutPanelController> = {}, settingsOpen = true) {
  const sample = layoutPanelCorpus.cases.mixed.before;
  const sheet = sample.projection.composition.sheets[0];
  const prepared = sample.queries[sheet.sheetId];
  const updateSettings = vi.fn(async () => true);
  const controller: LayoutPanelController = {
    visible: true, sheetId: sheet.sheetId, composition: sample.projection.composition,
    committing: false, query: prepared.query, previews: prepared.previews, error: null,
    toggle: vi.fn(), close: vi.fn(), preview: vi.fn(), cancelPreview: vi.fn(),
    apply: vi.fn(async () => true), updateSettings,
    lock: vi.fn(async () => true), unlock: vi.fn(async () => true),
    positionCount: sheet.frames.length, newFrameOrientation: "horizontal", configurePositions: vi.fn(),
    ...overrides,
  };
  render(<LayoutPanel controller={controller} sheet={sheet} mediaPreviewUrls={{}} presentationUnit={unit} />);
  if (settingsOpen) fireEvent.click(screen.getByRole("button", { name: "Ajustes" }));
  return { updateSettings, controller };
}

test("the lock remains actionable when extra positions disable the preview body", () => {
  const prepared = structuredClone(layoutPanelCorpus.cases.mixed.before.queries["sheet-001"].query);
  prepared.frameCount -= 1;
  const { controller } = panel("mm", { query: prepared }, false);
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
  const { controller } = panel("mm", { query: prepared }, false);
  for (const candidate of screen.getAllByRole("button", { name: /^Aplicar/ })) expect(candidate).toBeDisabled();
  const unlock = screen.getByRole("button", { name: "Destravar Layout da Lâmina 01" });
  expect(unlock).toBeEnabled();
  fireEvent.click(unlock);
  expect(controller.unlock).toHaveBeenCalledOnce();
  expect(controller.lock).not.toHaveBeenCalled();
});

test.each([
  ["mm", "Margem (mm)", "25", 25000],
  ["cm", "Margem (cm)", "2,5", 25000],
  ["in", "Margem (pol)", "1", 25400],
] as const)("Layout settings use %s without rounding untouched physical values", (unit, label, value, marginUm) => {
  const { updateSettings } = panel(unit);
  fireEvent.change(screen.getByRole("textbox", { name: label }), { target: { value } });
  fireEvent.change(screen.getByRole("combobox", { name: "Permitir" }), { target: { value: "pagesOnly" } });
  fireEvent.click(screen.getByRole("button", { name: "Atualizar sugestões" }));
  expect(updateSettings).toHaveBeenCalledExactlyOnceWith({ permission: "pagesOnly", marginUm,
    gapUm: 5000, minimumSideUm: 20000 });
});

test("invalid physical fields immediately expose the shared tooltip and never send settings", () => {
  const { updateSettings } = panel("mm");
  const input = screen.getByRole("textbox", { name: "Menor lado (mm)" });
  fireEvent.change(input, { target: { value: "0" } });
  expect(input).toHaveAttribute("aria-invalid", "true");
  expect(input).toHaveAccessibleDescription("O menor lado deve ser maior que zero.");
  expect(screen.getByRole("tooltip")).toHaveTextContent("maior que zero");
  fireEvent.click(screen.getByRole("button", { name: "Atualizar sugestões" }));
  expect(updateSettings).not.toHaveBeenCalled();
  fireEvent.change(input, { target: { value: "20" } });
  expect(input).not.toHaveAttribute("aria-invalid");
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
});
