import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type {
  NewProjectConfiguration,
  ProjectConfigurationValidationOutcome,
  ProjectLaunchOutcome,
} from "./application/globalProjectPort";
import { NewProjectFlow } from "./NewProjectFlow";

function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

function validConfiguration(): Promise<ProjectConfigurationValidationOutcome> {
  return Promise.resolve({ status: "valid" });
}

test("validates and creates with the complete neutral configuration", async () => {
  const user = userEvent.setup();
  const onValidate = vi.fn(validConfiguration);
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={onValidate}
    />,
  );

  expect(screen.getByRole("combobox", { name: "Unidade" })).toHaveValue(
    "mm",
  );
  expect(
    screen.getByRole("textbox", { name: "Largura da Lâmina" }),
  ).toHaveValue("600");
  expect(
    screen.getByRole("textbox", { name: "Altura da Lâmina" }),
  ).toHaveValue("300");
  expect(screen.getByRole("textbox", { name: "DPI" })).toHaveValue("300");
  expect(
    screen.getByRole("textbox", { name: "Quantidade de Lâminas" }),
  ).toHaveValue("2");
  expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue("3");
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("3");
  expect(
    screen.getByText(
      "Lâmina 600 × 300 mm · Página 300 × 300 mm · 300 DPI",
    ),
  ).toBeInTheDocument();
  expect(
    screen.queryByLabelText("Reprodução da Lâmina"),
  ).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Próximo" }));
  expect(
    await screen.findByRole("heading", { name: "Personalização" }),
  ).toBeInTheDocument();

  const expectedConfiguration = {
    document: {
      displayUnit: "mm",
      sheetWidthUm: 600_000,
      sheetHeightUm: 300_000,
      dpi: 300,
      bleedUm: 3_000,
      safetyUm: 3_000,
    },
    structure: {
      sheetCount: 2,
      firstSheet: "double",
      lastSheet: "double",
    },
  } satisfies NewProjectConfiguration;
  expect(onValidate).toHaveBeenCalledWith(expectedConfiguration);

  await user.click(screen.getByRole("button", { name: "Criar" }));
  expect(onCreate).toHaveBeenCalledWith(expectedConfiguration);
});

test("converts periodic display values without changing physical values and keeps the chosen proportion", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={validConfiguration}
    />,
  );

  const unit = screen.getByRole("combobox", { name: "Unidade" });
  const width = screen.getByRole("textbox", {
    name: "Largura da Lâmina",
  });
  const height = screen.getByRole("textbox", {
    name: "Altura da Lâmina",
  });
  await user.selectOptions(unit, "in");
  expect(width).toHaveValue("23.622047244094488");
  expect(height).toHaveValue("11.811023622047244");
  await user.selectOptions(unit, "mm");
  expect(width).toHaveValue("600");
  expect(height).toHaveValue("300");

  await user.selectOptions(unit, "cm");
  fireEvent.change(width, { target: { value: "50.8" } });
  fireEvent.change(height, { target: { value: "25.4" } });
  fireEvent.change(screen.getByRole("textbox", { name: "DPI" }), {
    target: { value: "600" },
  });
  fireEvent.change(
    screen.getByRole("textbox", { name: "Quantidade de Lâminas" }),
    { target: { value: "4" } },
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Primeira Lâmina" }),
    "singlePage",
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Sangria" }), {
    target: { value: "0" },
  });

  await user.click(screen.getByRole("button", { name: "Próximo" }));
  expect(
    await screen.findByLabelText("Prévia do formato da Lâmina"),
  ).toHaveStyle({ aspectRatio: "508000 / 254000" });
  await user.click(screen.getByRole("button", { name: "Criar" }));
  expect(onCreate).toHaveBeenCalledWith({
    document: {
      displayUnit: "cm",
      sheetWidthUm: 508_000,
      sheetHeightUm: 254_000,
      dpi: 600,
      bleedUm: 0,
      safetyUm: 3_000,
    },
    structure: {
      sheetCount: 4,
      firstSheet: "singlePage",
      lastSheet: "double",
    },
  } satisfies NewProjectConfiguration);
});

test("blocks locally unrepresentable text, then revalidates through the Core after correction", async () => {
  const user = userEvent.setup();
  const onValidate = vi.fn(validConfiguration);

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={onValidate}
    />,
  );
  const width = screen.getByRole("textbox", {
    name: "Largura da Lâmina",
  });
  fireEvent.change(width, { target: { value: "60.0001" } });
  expect(screen.queryByText(/micrômetros inteiros/i)).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Próximo" }));
  expect(width).toHaveFocus();
  expect(screen.getByText(/micrômetros inteiros/i)).toBeInTheDocument();
  expect(onValidate).not.toHaveBeenCalled();

  fireEvent.change(width, { target: { value: "600" } });
  await waitFor(() => expect(onValidate).toHaveBeenCalledOnce());
  await waitFor(() =>
    expect(screen.queryByText(/micrômetros inteiros/i)).not.toBeInTheDocument(),
  );
  expect(
    screen.getByRole("heading", { name: "Dimensões" }),
  ).toBeInTheDocument();
});

test("shows every Core error, focuses the first field and refreshes errors after editing", async () => {
  const user = userEvent.setup();
  const onValidate = vi
    .fn<() => Promise<ProjectConfigurationValidationOutcome>>()
    .mockResolvedValueOnce({
      status: "invalid",
      errors: [
        "sheetHeightNotPositive",
        "dpiOutOfRange",
        "sheetCountTooSmall",
      ],
    })
    .mockResolvedValue({ status: "valid" });

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={onValidate}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Próximo" }));

  expect(await screen.findByText(/altura.*maior que zero/i)).toBeInTheDocument();
  expect(screen.getByText(/DPI inteiro entre 1 e 1\.200/i)).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();
  expect(
    screen.getByRole("textbox", { name: "Altura da Lâmina" }),
  ).toHaveFocus();

  fireEvent.change(
    screen.getByRole("textbox", { name: "Altura da Lâmina" }),
    { target: { value: "250" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(screen.queryByText(/altura.*maior que zero/i)).not.toBeInTheDocument(),
  );
  expect(screen.queryByText(/DPI inteiro entre/i)).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Dimensões" }),
  ).toBeInTheDocument();
});

test("preserves errors from untouched fields during live validation", async () => {
  const user = userEvent.setup();
  const liveValidation = deferred<ProjectConfigurationValidationOutcome>();
  const onValidate = vi
    .fn<() => Promise<ProjectConfigurationValidationOutcome>>()
    .mockResolvedValueOnce({
      status: "invalid",
      errors: ["dpiOutOfRange", "sheetCountTooSmall"],
    })
    .mockReturnValueOnce(liveValidation.promise);

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={onValidate}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Próximo" }));

  expect(
    await screen.findByText(/DPI inteiro entre 1 e 1\.200/i),
  ).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();

  fireEvent.change(
    screen.getByRole("textbox", { name: "Altura da Lâmina" }),
    { target: { value: "250" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));
  expect(screen.getByText(/DPI inteiro entre 1 e 1\.200/i)).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();

  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura da Lâmina" }),
    { target: { value: "600.0001" } },
  );
  expect(screen.getByText(/micrômetros inteiros/i)).toBeInTheDocument();
  expect(screen.getByText(/DPI inteiro entre 1 e 1\.200/i)).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();
  expect(onValidate).toHaveBeenCalledTimes(2);

  await act(async () => {
    liveValidation.resolve({ status: "valid" });
  });
  expect(screen.getByText(/micrômetros inteiros/i)).toBeInTheDocument();
  expect(screen.getByText(/DPI inteiro entre 1 e 1\.200/i)).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();
});

test("ignores a late validation response after a newer edit", async () => {
  const user = userEvent.setup();
  const first = deferred<ProjectConfigurationValidationOutcome>();
  const second = deferred<ProjectConfigurationValidationOutcome>();
  const onValidate = vi
    .fn<() => Promise<ProjectConfigurationValidationOutcome>>()
    .mockReturnValueOnce(first.promise)
    .mockReturnValueOnce(second.promise);

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={onValidate}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Próximo" }));
  await waitFor(() => expect(onValidate).toHaveBeenCalledOnce());
  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura da Lâmina" }),
    { target: { value: "500" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));

  await act(async () => {
    second.resolve({
      status: "invalid",
      errors: ["sheetWidthNotEven"],
    });
  });
  expect(screen.getByText(/micrômetros pares/i)).toBeInTheDocument();

  await act(async () => {
    first.resolve({ status: "valid" });
  });
  expect(screen.getByText(/micrômetros pares/i)).toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Dimensões" }),
  ).toBeInTheDocument();
});

test("blocks navigation and shows an actionable operational validation failure", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={async () => ({
        status: "failed",
        error: {
          code: "validation_unavailable",
          message: "A validação está indisponível.",
          action: "Tente novamente.",
        },
      })}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Próximo" }));

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "A validação está indisponível.",
  );
  expect(screen.getByRole("alert")).toHaveTextContent("Tente novamente.");
  expect(
    screen.getByRole("heading", { name: "Dimensões" }),
  ).toBeInTheDocument();
  expect(onCreate).not.toHaveBeenCalled();
});

test("cancels before validation or creation", async () => {
  const user = userEvent.setup();
  const onCancel = vi.fn();
  const onValidate = vi.fn(validConfiguration);
  const onCreate = vi.fn(async () => ({ status: "opened" as const }));

  render(
    <NewProjectFlow
      onCancel={onCancel}
      onCreate={onCreate}
      onValidate={onValidate}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(onCancel).toHaveBeenCalledOnce();
  expect(onValidate).not.toHaveBeenCalled();
  expect(onCreate).not.toHaveBeenCalled();
});

test("preserves the draft after native cancellation and structured creation failure", async () => {
  const user = userEvent.setup();
  const nativeCreation = deferred<ProjectLaunchOutcome>();
  const onCreate = vi
    .fn<() => Promise<ProjectLaunchOutcome>>()
    .mockReturnValueOnce(nativeCreation.promise)
    .mockResolvedValueOnce({
      status: "failed",
      error: {
        code: "destination_conflict",
        message: "Outro objeto passou a ocupar este destino.",
      },
    });

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={validConfiguration}
    />,
  );
  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura da Lâmina" }),
    { target: { value: "500" } },
  );
  await user.click(screen.getByRole("button", { name: "Próximo" }));
  await user.click(await screen.findByRole("button", { name: "Criar" }));
  await act(async () => nativeCreation.resolve({ status: "cancelled" }));

  await user.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByRole("textbox", { name: "Largura da Lâmina" }),
  ).toHaveValue("500");
  await user.click(screen.getByRole("button", { name: "Próximo" }));
  await user.click(await screen.findByRole("button", { name: "Criar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Outro objeto passou a ocupar este destino.",
  );
  await user.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByRole("textbox", { name: "Largura da Lâmina" }),
  ).toHaveValue("500");
});
