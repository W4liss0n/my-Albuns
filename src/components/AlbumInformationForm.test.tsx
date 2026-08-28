import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { useState, type ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import type {
  AlbumInformation,
  AlbumInformationValidation,
  DisplayUnit,
} from "../domain/project";
import { representativeProjection } from "../test/projectFixtures";
import { AlbumInformationForm } from "./AlbumInformationForm";

const validImpact = {
  sheetWidthPx: 7_087,
  pageWidthPx: 3_543,
  heightPx: 3_543,
};
const emptySheetStates = representativeProjection.state.album.sheets.map(
  (sheet) => ({ ...sheet, frames: [] }),
);

function renderForm({
  onApply = vi.fn(),
  onValidate = vi.fn(async () => ({ errors: [], impact: validImpact })),
  sheetStates = emptySheetStates,
}: {
  onApply?: ComponentProps<typeof AlbumInformationForm>["onApply"];
  onValidate?: (
    information: AlbumInformation,
  ) => Promise<AlbumInformationValidation>;
  sheetStates?: typeof representativeProjection.state.album.sheets;
} = {}) {
  const onPresentationUnitChange = vi.fn();
  function Harness() {
    const [ready, setReady] = useState(false);
    return (
      <>
        <AlbumInformationForm
          document={representativeProjection.state.document}
          formId="album-information-test"
          revision={representativeProjection.state.revision}
          sheetStates={sheetStates}
          onApply={onApply}
          onPresentationUnitChange={onPresentationUnitChange}
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
  return { ...view, onApply, onPresentationUnitChange, onValidate };
}

function ProjectionHarness({
  document,
  onApply = vi.fn(),
  onPresentationUnitChange,
  onValidate,
  revision = representativeProjection.state.revision,
  sheetStates,
}: {
  document: typeof representativeProjection.state.document;
  onApply?: ComponentProps<typeof AlbumInformationForm>["onApply"];
  onPresentationUnitChange: (unit: DisplayUnit | null) => void;
  onValidate: (
    information: AlbumInformation,
  ) => Promise<AlbumInformationValidation>;
  revision?: number;
  sheetStates: typeof representativeProjection.state.album.sheets;
}) {
  const [ready, setReady] = useState(false);
  return (
    <>
      <AlbumInformationForm
        document={document}
        formId="album-information-equivalent-projection"
        revision={revision}
        sheetStates={sheetStates}
        onApply={onApply}
        onPresentationUnitChange={onPresentationUnitChange}
        onReadyChange={setReady}
        onValidate={onValidate}
      />
      <button
        disabled={!ready}
        form="album-information-equivalent-projection"
        type="submit"
      >
        Aplicar
      </button>
    </>
  );
}

test("marks composed dimension changes as owned by the safe transformation flow", () => {
  renderForm({ sheetStates: representativeProjection.state.album.sheets });

  const dimensions = screen.getByRole("group", { name: "Dimensão da Lâmina" });
  for (const label of ["Largura", "Altura"]) {
    const input = within(dimensions).getByRole("textbox", { name: label });
    expect(input).toBeDisabled();
    expect(input.closest(".album-information-field")).toHaveAttribute(
      "data-placeholder-feature",
      "safe-sheet-dimension-change",
    );
  }
});

test("marks composed edge conversions as owned by the complete conversion flow", () => {
  renderForm({ sheetStates: representativeProjection.state.album.sheets });

  for (const label of ["Primeira Lâmina", "Última Lâmina"]) {
    const select = screen.getByRole("combobox", { name: label });
    expect(select).toBeDisabled();
    expect(select.closest(".album-information-field")).toHaveAttribute(
      "data-placeholder-feature",
      "convert-composed-edge",
    );
  }
});

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
      expect.objectContaining({
        baseline: expect.objectContaining({
          sheetWidthUm: 600_000,
          sheetHeightUm: 300_000,
        }),
        baselineRevision: representativeProjection.state.revision,
        value: {
          displayUnit: "mm",
          sheetWidthUm: 700_000,
          sheetHeightUm: 350_000,
          dpi: 240,
          bleedUm: 4_000,
          safetyUm: 6_000,
          firstSheet: "singlePage",
          lastSheet: "singlePage",
        },
      }),
      validImpact,
    ),
  );
});

test("publishes the pending display Unit and clears it on unmount", () => {
  const { onPresentationUnitChange, unmount } = renderForm();

  expect(onPresentationUnitChange).toHaveBeenLastCalledWith("mm");
  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "in" },
  });
  expect(onPresentationUnitChange).toHaveBeenLastCalledWith("in");

  unmount();
  expect(onPresentationUnitChange).toHaveBeenLastCalledWith(null);
});

test("restores edited entries to their last applied values", () => {
  renderForm();

  expect(
    screen.queryAllByRole("button", { name: /^Restaurar / }),
  ).toHaveLength(0);

  const dpi = screen.getByRole("textbox", { name: "DPI" });
  fireEvent.change(dpi, { target: { value: "240" } });
  fireEvent.click(screen.getByRole("button", { name: "Restaurar DPI" }));
  expect(dpi).toHaveValue("300");
  expect(
    screen.queryByRole("button", { name: "Restaurar DPI" }),
  ).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "cm" },
  });
  const width = screen.getByRole("textbox", { name: "Largura" });
  expect(width).toHaveValue("60");
  expect(
    screen.queryByRole("button", { name: "Restaurar Largura" }),
  ).not.toBeInTheDocument();

  fireEvent.change(width, { target: { value: "70" } });
  fireEvent.click(screen.getByRole("button", { name: "Restaurar Largura" }));
  expect(width).toHaveValue("60");
  expect(
    screen.queryByRole("button", { name: "Restaurar Largura" }),
  ).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "in" },
  });
  expect(width).toHaveValue("23.622");
  fireEvent.change(width, { target: { value: "23.621" } });
  fireEvent.change(width, { target: { value: "23.622" } });
  expect(
    screen.queryByRole("button", { name: "Restaurar Largura" }),
  ).not.toBeInTheDocument();
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

test("accepts the generated inch text again after a temporary exact edit", async () => {
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange = vi.fn<(unit: DisplayUnit | null) => void>();
  render(
    <ProjectionHarness
      document={{
        ...representativeProjection.state.document,
        displayUnit: "cm",
        sheetWidthUm: 400_000,
        sheetHeightUm: 300_000,
      }}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "in" },
  });
  const width = screen.getByRole("textbox", { name: "Largura" });
  expect(width).toHaveValue("15.748");

  fireEvent.change(width, { target: { value: "15.74" } });
  await waitFor(() =>
    expect(onValidate).toHaveBeenLastCalledWith(
      expect.objectContaining({ sheetWidthUm: 399_796 }),
    ),
  );

  fireEvent.change(width, { target: { value: "15.748" } });
  await waitFor(() =>
    expect(onValidate).toHaveBeenLastCalledWith(
      expect.objectContaining({ sheetWidthUm: 400_000 }),
    ),
  );
  expect(width).not.toHaveAttribute("aria-invalid", "true");
  expect(
    screen.queryByText("Informe uma medida válida em pol."),
  ).not.toBeInTheDocument();
});

test("keeps one grouped validation tooltip open until correction or outside click", async () => {
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

  const message = "Informe uma medida válida em pol.";
  expect(screen.getByRole("tooltip")).toHaveTextContent(message);
  expect(screen.getAllByRole("alert")).toHaveLength(1);
  const descriptionId = screen.getByRole("alert").id;
  for (const [name] of invalidMeasurements) {
    const input = screen.getByRole("textbox", { name });
    expect(input).toHaveAttribute("aria-describedby", descriptionId);
    expect(input).not.toHaveAttribute("title");
  }

  fireEvent.pointerDown(screen.getByRole("textbox", { name: "Largura" }));
  expect(screen.getByRole("tooltip")).toBeInTheDocument();
  fireEvent.pointerDown(screen.getByRole("heading", { name: "Áreas técnicas" }));
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  expect(screen.getByRole("textbox", { name: "Largura" })).toHaveAttribute(
    "aria-invalid",
    "true",
  );

  fireEvent.click(screen.getByRole("textbox", { name: "Largura" }));
  expect(screen.getByRole("tooltip")).toHaveTextContent(message);
  fireEvent.pointerDown(screen.getByRole("heading", { name: "Áreas técnicas" }));
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  fireEvent.focus(screen.getByRole("textbox", { name: "Altura" }));
  expect(screen.getByRole("tooltip")).toHaveTextContent(message);
  fireEvent.pointerDown(screen.getByRole("heading", { name: "Áreas técnicas" }));
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("textbox", { name: "Largura" }), {
    target: { value: "23.623" },
  });
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "mm" },
  });
  await waitFor(() =>
    expect(screen.queryByRole("tooltip")).not.toBeInTheDocument(),
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Largura" }), {
    target: { value: "600.0001" },
  });
  expect(await screen.findByRole("tooltip")).toHaveTextContent(
    "Informe uma medida válida em mm.",
  );
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

test("presents raster limits in the pending Unit instead of pixels", async () => {
  const onValidate = vi.fn(async (): Promise<AlbumInformationValidation> => ({
    errors: ["sheetWidthRasterOutOfRange"],
    impact: null,
  }));
  renderForm({ onValidate });

  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "cm" },
  });

  const tooltip = await screen.findByRole("tooltip");
  expect(tooltip).toHaveTextContent(
    "Para 300 DPI, informe a largura da Lâmina entre 0.0086 cm e 554.8672 cm.",
  );
  expect(tooltip).not.toHaveTextContent(/pixels?/i);
});

test("preserves an unapplied draft across a semantically equivalent projection", async () => {
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const view = render(
    <ProjectionHarness
      document={representativeProjection.state.document}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  view.rerender(
    <ProjectionHarness
      document={{ ...representativeProjection.state.document }}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets.map((sheet) => ({
        ...sheet,
      }))}
    />,
  );

  expect(screen.getByRole("textbox", { name: "DPI" })).toHaveValue("600");
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled();
});

test("revalidates a pending dimension draft when composed geometry arrives", async () => {
  let composed = false;
  const onValidate = vi.fn(
    async (): Promise<AlbumInformationValidation> =>
      composed
        ? {
            errors: ["sheetDimensionsRequireContentTransformation"],
            impact: null,
          }
        : { errors: [], impact: validImpact },
  );
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const view = render(
    <ProjectionHarness
      document={representativeProjection.state.document}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={emptySheetStates}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "Largura" }), {
    target: { value: "700" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Altura" }), {
    target: { value: "350" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  const validationsBeforeComposition = onValidate.mock.calls.length;

  composed = true;
  view.rerender(
    <ProjectionHarness
      document={representativeProjection.state.document}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();
  await waitFor(() =>
    expect(onValidate.mock.calls.length).toBeGreaterThan(
      validationsBeforeComposition,
    ),
  );
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();

  fireEvent.click(screen.getByRole("button", { name: "Restaurar Largura" }));
  fireEvent.click(screen.getByRole("button", { name: "Restaurar Altura" }));
  expect(screen.getByRole("textbox", { name: "Largura" })).toHaveValue("600");
  expect(screen.getByRole("textbox", { name: "Altura" })).toHaveValue("300");
});

test("lets a pending edge conversion be restored when edge content arrives", async () => {
  let composed = false;
  const onValidate = vi.fn(
    async (): Promise<AlbumInformationValidation> =>
      composed
        ? {
            errors: ["firstSheetConversionRequiresContentReorganization"],
            impact: null,
          }
        : { errors: [], impact: validImpact },
  );
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const view = render(
    <ProjectionHarness
      document={representativeProjection.state.document}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={emptySheetStates}
    />,
  );

  fireEvent.change(screen.getByRole("combobox", { name: "Primeira Lâmina" }), {
    target: { value: "singlePage" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  const validationsBeforeComposition = onValidate.mock.calls.length;

  composed = true;
  view.rerender(
    <ProjectionHarness
      document={representativeProjection.state.document}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();
  await waitFor(() =>
    expect(onValidate.mock.calls.length).toBeGreaterThan(
      validationsBeforeComposition,
    ),
  );
  expect(
    screen.getByRole("combobox", { name: "Primeira Lâmina" }),
  ).toBeDisabled();
  fireEvent.click(
    screen.getByRole("button", { name: "Restaurar Primeira Lâmina" }),
  );
  expect(
    screen.getByRole("combobox", { name: "Primeira Lâmina" }),
  ).toHaveValue("double");
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();
});

test("resets the draft when authoritative Album information really changes", async () => {
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const view = render(
    <ProjectionHarness
      document={representativeProjection.state.document}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );
  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "600" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );

  view.rerender(
    <ProjectionHarness
      document={{ ...representativeProjection.state.document, dpi: 240 }}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  await waitFor(() =>
    expect(screen.getByRole("textbox", { name: "DPI" })).toHaveValue("240"),
  );
  expect(screen.getByRole("button", { name: "Aplicar" })).toBeDisabled();
});

test("preserves edits made after submit when the applied Album information projection arrives", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumInformationForm>["onApply"]>(
    () => pendingApply,
  );
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const view = render(
    <ProjectionHarness
      document={representativeProjection.state.document}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());

  fireEvent.change(
    screen.getByRole("textbox", { name: "Área de segurança" }),
    { target: { value: "8" } },
  );
  view.rerender(
    <ProjectionHarness
      document={{ ...representativeProjection.state.document, dpi: 240 }}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  await waitFor(() =>
    expect(
      screen.getByRole("textbox", { name: "Área de segurança" }),
    ).toHaveValue("8"),
  );
  await act(async () => {
    finishApply(true);
    await pendingApply;
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
});

test("keeps post-submit edits through two successful History predecessors", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumInformationForm>["onApply"]>(
    () => pendingApply,
  );
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const baselineDocument = representativeProjection.state.document;
  const view = render(
    <ProjectionHarness
      document={baselineDocument}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());

  view.rerender(
    <ProjectionHarness
      document={{ ...baselineDocument, dpi: 240 }}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );
  fireEvent.change(
    screen.getByRole("textbox", { name: "Área de segurança" }),
    { target: { value: "8" } },
  );

  view.rerender(
    <ProjectionHarness
      document={baselineDocument}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 2}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  await waitFor(() =>
    expect(
      screen.getByRole("textbox", { name: "Área de segurança" }),
    ).toHaveValue("8"),
  );
  await act(async () => {
    finishApply(false);
    await pendingApply;
  });
});

test("reformats exact post-submit measurements when History changes only the presentation Unit", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumInformationForm>["onApply"]>(
    () => pendingApply,
  );
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const baselineDocument = representativeProjection.state.document;
  const view = render(
    <ProjectionHarness
      document={baselineDocument}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());
  fireEvent.change(
    screen.getByRole("textbox", { name: "Área de segurança" }),
    { target: { value: "8" } },
  );

  view.rerender(
    <ProjectionHarness
      document={{ ...baselineDocument, displayUnit: "cm" }}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  await waitFor(() =>
    expect(screen.getByRole("combobox", { name: "Unidade" })).toHaveValue(
      "cm",
    ),
  );
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("0.8");
  await act(async () => {
    finishApply(false);
    await pendingApply;
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));

  await waitFor(() => expect(onApply).toHaveBeenCalledTimes(2));
  expect(onApply.mock.calls[1]?.[0].value.safetyUm).toBe(8_000);
});

test("treats unit reformatting as equal while rebasing concurrent measurements", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumInformationForm>["onApply"]>(
    () => pendingApply,
  );
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const baselineDocument = representativeProjection.state.document;
  const view = render(
    <ProjectionHarness
      document={baselineDocument}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());
  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "cm" },
  });
  fireEvent.change(screen.getByRole("textbox", { name: "Sangria" }), {
    target: { value: "0.30" },
  });

  view.rerender(
    <ProjectionHarness
      document={{ ...baselineDocument, bleedUm: 5_000 }}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  await waitFor(() =>
    expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue(
      "0.5",
    ),
  );
  expect(screen.getByRole("combobox", { name: "Unidade" })).toHaveValue(
    "cm",
  );
  await act(async () => {
    finishApply(false);
    await pendingApply;
  });
});

test("continues following unedited concurrent fields across multiple predecessors", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumInformationForm>["onApply"]>(
    () => pendingApply,
  );
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const baselineDocument = representativeProjection.state.document;
  const view = render(
    <ProjectionHarness
      document={baselineDocument}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());

  view.rerender(
    <ProjectionHarness
      document={{ ...baselineDocument, bleedUm: 5_000 }}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );
  await waitFor(() =>
    expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue("5"),
  );
  view.rerender(
    <ProjectionHarness
      document={{ ...baselineDocument, bleedUm: 7_000 }}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 2}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  await waitFor(() =>
    expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue("7"),
  );
  await act(async () => {
    finishApply(false);
    await pendingApply;
  });
});

test("clears a normalized invalid measurement marker when Unidade changes", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumInformationForm>["onApply"]>(
    () => pendingApply,
  );
  const onValidate = vi.fn(async () => ({ errors: [], impact: validImpact }));
  const onPresentationUnitChange =
    vi.fn<(unit: DisplayUnit | null) => void>();
  const baselineDocument = representativeProjection.state.document;
  const view = render(
    <ProjectionHarness
      document={baselineDocument}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());
  fireEvent.change(screen.getByRole("textbox", { name: "Sangria" }), {
    target: { value: "abc" },
  });
  fireEvent.change(screen.getByRole("combobox", { name: "Unidade" }), {
    target: { value: "cm" },
  });

  view.rerender(
    <ProjectionHarness
      document={{ ...baselineDocument, bleedUm: 5_000 }}
      onApply={onApply}
      onPresentationUnitChange={onPresentationUnitChange}
      onValidate={onValidate}
      revision={representativeProjection.state.revision + 1}
      sheetStates={representativeProjection.state.album.sheets}
    />,
  );

  await waitFor(() =>
    expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue(
      "0.5",
    ),
  );
  await act(async () => {
    finishApply(false);
    await pendingApply;
  });
});

test("keeps the full Album information draft after a pending submit is cancelled", async () => {
  let finishApply!: (completed: boolean) => void;
  const pendingApply = new Promise<boolean>((resolve) => {
    finishApply = resolve;
  });
  const onApply = vi.fn<ComponentProps<typeof AlbumInformationForm>["onApply"]>(
    () => pendingApply,
  );
  renderForm({ onApply });

  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "240" },
  });
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
  fireEvent.click(screen.getByRole("button", { name: "Aplicar" }));
  await waitFor(() => expect(onApply).toHaveBeenCalledOnce());
  fireEvent.change(
    screen.getByRole("textbox", { name: "Área de segurança" }),
    { target: { value: "8" } },
  );

  await act(async () => {
    finishApply(false);
    await pendingApply;
  });

  expect(screen.getByRole("textbox", { name: "DPI" })).toHaveValue("240");
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("8");
  await waitFor(() =>
    expect(screen.getByRole("button", { name: "Aplicar" })).toBeEnabled(),
  );
});
