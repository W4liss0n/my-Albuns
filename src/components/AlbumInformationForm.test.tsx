import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { useState } from "react";
import { expect, test, vi } from "vitest";

import type {
  AlbumInformation,
  AlbumInformationValidation,
} from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import { AlbumInformationForm } from "./AlbumInformationForm";

const validImpact = {
  sheetWidthPx: 7_087,
  pageWidthPx: 3_543,
  heightPx: 3_543,
};

function renderForm({
  onApply = vi.fn(),
  onValidate = vi.fn(async () => ({ errors: [], impact: validImpact })),
}: {
  onApply?: (
    information: AlbumInformation,
    baseline: AlbumInformation,
    impact: typeof validImpact,
  ) => void | Promise<unknown>;
  onValidate?: (
    information: AlbumInformation,
  ) => Promise<AlbumInformationValidation>;
} = {}) {
  function Harness() {
    const [ready, setReady] = useState(false);
    return (
      <>
        <AlbumInformationForm
          document={representativeProjection.state.document}
          formId="album-information-test"
          sheetStates={representativeProjection.state.album.sheets}
          onApply={onApply}
          onReadyChange={setReady}
          onValidate={onValidate}
        />
        <button disabled={!ready} form="album-information-test" type="submit">
          Aplicar
        </button>
      </>
    );
  }
  const view = render(
    <Harness />,
  );
  return { ...view, onApply, onValidate };
}

test("edits every Album information field and submits one complete candidate", async () => {
  const onApply = vi.fn();
  renderForm({ onApply });

  fireEvent.change(screen.getByRole("combobox", { name: "Primeira Lâmina" }), {
    target: { value: "singlePage" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "Última Lâmina" }), {
    target: { value: "singlePage" },
  });
  const sheetDimension = screen.getByRole("group", {
    name: "Dimensão da Lâmina",
  });
  fireEvent.change(within(sheetDimension).getByRole("textbox", { name: "Largura" }), {
    target: { value: "700" },
  });
  fireEvent.change(within(sheetDimension).getByRole("textbox", { name: "Altura" }), {
    target: { value: "350" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Sangria" }), {
    target: { value: "4" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Área de segurança" }), {
    target: { value: "6" },
  });

  await waitFor(() => expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));

  await waitFor(() =>
    expect(onApply).toHaveBeenCalledWith(
      {
        displayUnit: "mm",
        sheetWidthUm: 700_000,
        sheetHeightUm: 350_000,
        dpi: 240,
        bleedUm: 4_000,
        safetyUm: 6_000,
        firstSheet: "singlePage",
        lastSheet: "singlePage",
      },
      expect.objectContaining({
        sheetWidthUm: 600_000,
        sheetHeightUm: 300_000,
      }),
      validImpact,
    ),
  );
});

test("changing Unidade converts presentation without changing physical dimensions", async () => {
  const { onValidate } = renderForm();

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "cm" },
  });

  const sheetDimension = screen.getByRole("group", {
    name: "Dimensão da Lâmina",
  });
  expect(within(sheetDimension).getByRole("textbox", { name: "Largura" })).toHaveValue("60");
  expect(within(sheetDimension).getByRole("textbox", { name: "Altura" })).toHaveValue("30");
  await waitFor(() =>
    expect(onValidate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        displayUnit: "cm",
        sheetWidthUm: 600_000,
        sheetHeightUm: 300_000,
      }),
    ),
  );

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "in" },
  });
  expect(
    within(sheetDimension).getByRole("textbox", { name: "Largura" }),
  ).toHaveValue("23.622");
  await waitFor(() =>
    expect(onValidate).toHaveBeenLastCalledWith(
      expect.objectContaining({
        displayUnit: "in",
        sheetWidthUm: 600_000,
        sheetHeightUm: 300_000,
      }),
    ),
  );
});

test("collapses repeated measurement errors into one hover tooltip", () => {
  renderForm();

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "in" },
  });
  const invalidMeasurements = [
    ["Largura", "23.621"],
    ["Altura", "11.812"],
    ["Sangria", "0.119"],
    ["Área de segurança", "0.117"],
  ] as const;
  for (const [name, value] of invalidMeasurements) {
    fireEvent.change(screen.getByRole("textbox", { name }), {
      target: { value },
    });
  }

  const message =
    "Informe uma medida decimal que corresponda a micrômetros inteiros.";
  expect(screen.getAllByText(message)).toHaveLength(1);
  expect(screen.getAllByRole("alert")).toHaveLength(1);
  for (const [name] of invalidMeasurements) {
    expect(
      screen.getByRole("textbox", { name }),
    ).toHaveAttribute("title", message);
  }
});

test("keeps the calculated Page dimension visible when DPI is invalid", () => {
  renderForm();

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "inválido" },
  });

  const pageDimension = screen.getByRole("group", {
    name: "Dimensão da Página",
  });
  expect(within(pageDimension).getByLabelText("Largura")).toHaveTextContent(
    "300 mm",
  );
  expect(within(pageDimension).getByLabelText("Altura")).toHaveTextContent(
    "300 mm",
  );
});

test("shows validation from the core and blocks Apply", async () => {
  const onValidate = vi.fn(async (): Promise<AlbumInformationValidation> => ({
    errors: ["bleedEliminatesCutArea"],
    impact: null,
  }));
  renderForm({ onValidate });

  fireEvent.change(screen.getByRole("textbox", { name: "Sangria" }), {
    target: { value: "160" },
  });

  expect(await screen.findByText("A Sangria deve manter uma Área de corte positiva.")).toBeVisible();
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();
});
