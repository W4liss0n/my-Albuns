import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { DisplayUnit } from "../domain/project";
import { layoutPanelCorpus } from "../test/layoutPanelPreview";
import { LayoutPanel } from "./LayoutPanel";
import type { LayoutPanelController } from "./useLayoutPanel";

function panel(unit: DisplayUnit) {
  const sample = layoutPanelCorpus.cases.mixed.before;
  const sheet = sample.projection.composition.sheets[0];
  const prepared = sample.queries[sheet.sheetId];
  const updateSettings = vi.fn(async () => true);
  const controller: LayoutPanelController = {
    visible: true, sheetId: sheet.sheetId, composition: sample.projection.composition,
    committing: false, query: prepared.query, previews: prepared.previews, error: null,
    toggle: vi.fn(), close: vi.fn(), preview: vi.fn(), cancelPreview: vi.fn(),
    apply: vi.fn(async () => true), updateSettings,
  };
  render(<LayoutPanel controller={controller} sheet={sheet} mediaPreviewUrls={{}} presentationUnit={unit} />);
  fireEvent.click(screen.getByRole("button", { name: "Ajustes" }));
  return { updateSettings };
}

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
