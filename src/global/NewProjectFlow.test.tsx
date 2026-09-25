import { chooseColor } from "../test/colorPicker";
import { rasterLimitsAt300Dpi } from "../test/projectConfigurationFixtures";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ComponentProps } from "react";
import { expect, test, vi } from "vitest";

import type {
  NewProjectConfiguration,
  ProjectConfigurationValidationOutcome,
  ProjectLaunchOutcome,
  ProvisionalDecorativeSelection,
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
import type { NewProjectCreationConfiguration } from "./application/newProjectPersonalization";
import { NewProjectFlow as ProductNewProjectFlow } from "./NewProjectFlow";

type NewProjectFlowProps = ComponentProps<typeof ProductNewProjectFlow>;

function NewProjectFlow({
  onOperationalFailure = async () => undefined,
  ...props
}: Omit<NewProjectFlowProps, "onOperationalFailure"> &
  Partial<Pick<NewProjectFlowProps, "onOperationalFailure">>) {
  return (
    <ProductNewProjectFlow
      {...props}
      onOperationalFailure={onOperationalFailure}
    />
  );
}

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

function selectedDecorative(
  selection: ProvisionalDecorativeSelection,
): ProvisionalDecorativeSelectionOutcome {
  return { status: "selected", selection };
}

const neutralVisualDefaults = {
  background: {
    scope: "bothSides" as const,
    both: { kind: "color" as const, rgb: "#FFFFFF" },
  },
  overlay: { scope: "bothSides" as const, both: null },
  frameBorder: { kind: "none" as const },
};

test("keeps the preview panel shared while the sheet content changes", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  const previewPanel = () =>
    screen.getByRole("region", { name: "Prévia da lâmina aberta" });

  expect(within(previewPanel()).getByText("Lâmina aberta")).toBeVisible();
  expect(within(previewPanel()).getByText(/18 lâminas/)).toBeVisible();
  expect(
    within(previewPanel()).getByText("Proporção real da lâmina aberta."),
  ).toBeVisible();
  expect(
    within(previewPanel()).getByLabelText("Guias de dobra, corte e segurança da lâmina"),
  ).toBeInTheDocument();
  expect(
    within(previewPanel()).getByRole("img", { name: "Prévia das Dimensões" }),
  ).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Continuar" }));

  expect(within(previewPanel()).getByText("Lâmina aberta")).toBeVisible();
  expect(within(previewPanel()).getByText(/18 lâminas/)).toBeVisible();
  expect(
    within(previewPanel()).getByText("Proporção real da lâmina aberta."),
  ).toBeVisible();
  expect(
    within(previewPanel()).getByLabelText("Guias de dobra, corte e segurança da lâmina"),
  ).toBeInTheDocument();
  expect(
    within(previewPanel()).getByRole("img", {
      name: "Reprodução da lâmina",
    }),
  ).toBeInTheDocument();
  expect(
    within(previewPanel()).queryByRole("img", {
      name: "Prévia das Dimensões",
    }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByText("Ambos os lados", { selector: ".ui-section-eyebrow" }),
  ).toBeVisible();
});

test("leads the panel of both steps with the preset control, leaving the steps alone in the header", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  const steps = screen.getByRole("list", { name: "Etapas da criação" });
  const header = steps.closest("header");
  expect(header).not.toBeNull();
  const panelPreset = () => screen.getByRole("combobox", { name: "Modelo inicial" });
  expect(header).not.toContainElement(panelPreset());
  expect(panelPreset().closest(".new-project-panel")).not.toBeNull();

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(
    await screen.findByRole("combobox", { name: "Modelo inicial" }),
  ).toBeVisible();
  expect(header).not.toContainElement(panelPreset());
  expect(panelPreset().closest(".new-project-panel")).not.toBeNull();
});

test("moves keyboard focus with the current New Project step", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  const currentStep = () => {
    const step = screen
      .getByRole("list", { name: "Etapas da criação" })
      .querySelector<HTMLElement>('[aria-current="step"]');
    if (!step) throw new Error("Etapa atual não encontrada.");
    return step;
  };

  expect(currentStep()).toHaveTextContent("Configurações");
  expect(currentStep()).toHaveFocus();

  const continueAction = screen.getByRole("button", { name: "Continuar" });
  continueAction.focus();
  await user.keyboard("{Enter}");

  await screen.findByRole("heading", { name: "Personalização" });
  expect(currentStep()).toHaveTextContent("Personalização");
  expect(currentStep()).toHaveFocus();

  const backAction = screen.getByRole("button", { name: "Voltar" });
  backAction.focus();
  await user.keyboard("{Enter}");

  expect(currentStep()).toHaveTextContent("Configurações");
  expect(currentStep()).toHaveFocus();
});

test("keeps every New Project action label in its accessible name", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const actionForVisibleLabel = (label: string) => {
    const action = screen.getByText(label, { exact: true }).closest("button");
    if (!action) throw new Error(`Ação visível não encontrada: ${label}`);
    return action;
  };

  const create = actionForVisibleLabel("Criar projeto");
  expect(create).toHaveAccessibleName("Criar projeto");

  // Background and overlay share the same visible action.
  const [chooseBackground, chooseOverlay] = screen.getAllByText("Escolher imagem…", { exact: true })
    .map(label => label.closest("button"));
  expect(chooseBackground).toHaveAccessibleName(/Escolher imagem/);
  expect(chooseOverlay).toHaveAccessibleName(/Escolher imagem/);
  expect(screen.queryByText("Usar imagem…")).not.toBeInTheDocument();
});

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

  expect(screen.getByRole("button", { name: "mm" })).toHaveAttribute(
    "aria-pressed",
    "true",
  );
  expect(
    screen.getByRole("textbox", { name: "Largura" }),
  ).toHaveValue("300");
  expect(
    screen.getByRole("textbox", { name: "Altura" }),
  ).toHaveValue("300");
  expect(screen.getByRole("textbox", { name: "Resolução" })).toHaveValue("300");
  expect(
    screen.getByRole("textbox", { name: "Lâminas" }),
  ).toHaveValue("18");
  expect(
    screen
      .getByRole("combobox", { name: "Modelo inicial" })
      .closest("section"),
  ).toHaveAttribute("data-placeholder-feature", "new-project-presets");
  await user.click(
    screen.getByRole("button", {
      name: "Aumentar quantidade de lâminas",
    }),
  );
  expect(
    screen.getByRole("textbox", { name: "Lâminas" }),
  ).toHaveValue("20");
  await user.click(
    screen.getByRole("button", {
      name: "Diminuir quantidade de lâminas",
    }),
  );
  expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue("3");
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("3");
  expect(
    screen.queryByText(
      "Lâmina 600 × 300 mm · página 300 × 300 mm · 300 DPI",
    ),
  ).not.toBeInTheDocument();
  expect(screen.queryByText("Configurações avançadas")).not.toBeInTheDocument();
  for (const group of ["Documento", "Dimensão da lâmina fechada", "Áreas técnicas", "Estrutura"]) {
    expect(screen.getByRole("heading", { name: group })).toBeVisible();
  }
  expect(
    screen.queryByText(
      "Medidas, sangria e Área de segurança valem para o álbum inteiro e podem ser alteradas depois nas Configurações do projeto.",
    ),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByLabelText("Reprodução da lâmina"),
  ).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Continuar" }));
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
      sheetCount: 18,
      firstSheet: "double",
      lastSheet: "double",
    },
  } satisfies NewProjectConfiguration;
  expect(onValidate).toHaveBeenCalledWith(expectedConfiguration);

  await user.click(screen.getByRole("button", { name: "Criar projeto" }));
  expect(onCreate).toHaveBeenCalledWith({
    ...expectedConfiguration,
    visualDefaults: neutralVisualDefaults,
    frameGapUm: 5_000,
  });
});

test("isolates cancellation and keeps backwards navigation beside the primary action", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  const footerActionNames = () => {
    const footer = screen
      .getByRole("button", { name: "Cancelar" })
      .closest("footer");
    if (!footer) throw new Error("Rodapé da criação não encontrado.");

    return within(footer)
      .getAllByRole("button")
      .map(
        (button) =>
          button.getAttribute("aria-label") ?? button.textContent?.trim(),
      );
  };

  expect(footerActionNames()).toEqual(["Cancelar", "Continuar"]);

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await screen.findByRole("button", { name: "Criar projeto" });

  expect(footerActionNames()).toEqual([
    "Cancelar",
    "Voltar",
    "Criar projeto",
  ]);
});

test("creates from the neutral visual defaults without copying the demonstrative Frames", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn<
    (
      configuration: NewProjectCreationConfiguration,
    ) => Promise<ProjectLaunchOutcome>
  >(async () => ({ status: "cancelled" }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={validConfiguration}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Continuar" }));

  expect(
    await screen.findByRole("img", { name: "Reprodução da lâmina" }),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("Fundo branco")).toHaveAttribute(
    "fill",
    "#FFFFFF",
  );
  const demonstrativeFrames = screen.getAllByLabelText(
    /Quadro de exemplo [12], lado (esquerdo|direito)/,
  );
  expect(demonstrativeFrames).toHaveLength(4);
  for (const frame of demonstrativeFrames) {
    expect(frame).toHaveAttribute("fill", "#7A684E");
    expect(frame).toHaveAttribute("fill-opacity", "0.24");
    expect(frame).toHaveAttribute("stroke", "none");
  }
  expect(screen.getByLabelText("Cor do fundo")).toHaveAttribute("title", "#FFFFFF");
  expect(
    screen.queryByRole("checkbox", { name: "Borda dos quadros" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("slider", { name: "Espessura da borda padrão" }),
  ).toHaveValue("0");
  expect(screen.getByRole("slider", { name: "Espessura da borda padrão" })).toHaveAttribute("aria-valuetext", "sem borda");
  const borderColors = within(
    screen.getByRole("group", { name: "Cores da borda" }),
  ).getAllByRole("button");
  expect(
    screen.getByRole("group", { name: "Cores da borda" }),
  ).toHaveClass("new-project-color-swatches");
  expect(borderColors).toHaveLength(4);
  expect(borderColors[0]).toHaveAttribute("aria-pressed", "true");
  expect(
    screen.getByRole("slider", { name: "Espaço entre quadros" }),
  ).toHaveValue("5000");
  expect(screen.getByRole("spinbutton", { name: "Espaço entre quadros em mm" })).toHaveValue("5");
  expect(
    screen
      .getByRole("slider", { name: "Espaço entre quadros" })
      .closest("div"),
  ).not.toHaveAttribute("data-placeholder-feature");
  const secondFrame = screen.getByLabelText("Quadro de exemplo 2, lado esquerdo");
  const initialSecondFrameX = Number(secondFrame.getAttribute("x"));
  fireEvent.change(
    screen.getByRole("slider", { name: "Espaço entre quadros" }),
    { target: { value: "18000" } },
  );
  expect(
    screen.getByRole("slider", { name: "Espaço entre quadros" }),
  ).toHaveValue("18000");
  expect(screen.getByRole("spinbutton", { name: "Espaço entre quadros em mm" })).toHaveValue("18");
  expect(Number(secondFrame.getAttribute("x"))).toBeGreaterThan(
    initialSecondFrameX,
  );
  expect(screen.getByLabelText("Fundo de ambos os lados")).toHaveAttribute(
    "fill",
    "#FFFFFF",
  );

  await user.click(screen.getByRole("button", { name: "Criar projeto" }));

  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      visualDefaults: neutralVisualDefaults,
      frameGapUm: 18_000,
    }),
  );
  expect(JSON.stringify(onCreate.mock.calls[0]?.[0])).not.toContain(
    '"frames"',
  );
  expect(JSON.stringify(onCreate.mock.calls[0]?.[0])).not.toContain(
    '"frameGap"',
  );
});

test("formats the Project Frame spacing in the configured Unit", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  await user.click(screen.getByRole("button", { name: "cm" }));
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  expect(
    screen.getByRole("slider", { name: "Espaço entre quadros" }),
  ).toHaveValue("5000");
  expect(screen.getByRole("spinbutton", { name: "Espaço entre quadros em cm" })).toHaveValue("0.5");
});

test.each([
  ["mm", "1,27", "5,08"],
  ["cm", "0,127", "0,508"],
  ["pol", "0,05", "0,2"],
])("keeps a custom border color and exact typed measures through Back/Create in %s", async (unit, borderText, gapText) => {
  const user = userEvent.setup();
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));
  render(<NewProjectFlow onCancel={vi.fn()} onCreate={onCreate} onValidate={validConfiguration} />);
  await user.click(screen.getByRole("button", { name: unit }));
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  chooseColor("da borda", "#4B7286");
  const width = screen.getByRole("spinbutton", { name: `Espessura da borda padrão em ${unit}` });
  fireEvent.change(width, { target: { value: borderText } });
  fireEvent.keyDown(width, { key: "Enter" });
  const gap = screen.getByRole("spinbutton", { name: `Espaço entre quadros em ${unit}` });
  fireEvent.change(gap, { target: { value: gapText } });
  fireEvent.keyDown(gap, { key: "Enter" });
  await user.click(screen.getByRole("button", { name: "Voltar" }));
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(screen.getByRole("button", { name: "Cor da borda" })).toHaveAttribute("title", "#4B7286");
  await user.click(screen.getByRole("button", { name: "Criar projeto" }));
  expect(onCreate).toHaveBeenCalledWith(expect.objectContaining({
    frameGapUm: 5_080,
    visualDefaults: expect.objectContaining({ frameBorder: { kind: "solid", rgb: "#4B7286", widthUm: 1_270 } }),
  }));
});

test("previews the pointed side even when it already belongs to the fixed scope", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const left = await screen.findByRole("button", { name: "Página esquerda" });
  const right = screen.getByRole("button", { name: "Página direita" });
  fireEvent.pointerEnter(left);
  expect(
    screen.getByLabelText("Pré-seleção do lado esquerdo"),
  ).toHaveAttribute("data-scope", "left");
  expect(screen.getByRole("button", { name: "Ambos os lados" })).toHaveAttribute("aria-pressed", "true");

  await user.click(left);
  fireEvent.pointerEnter(left);
  expect(
    screen.getByLabelText("Pré-seleção do lado esquerdo"),
  ).toHaveAttribute("data-scope", "left");

  fireEvent.pointerEnter(right);
  expect(screen.getByLabelText("Pré-seleção do lado direito")).toHaveAttribute(
    "data-scope",
    "right",
  );
  expect(left).toHaveAttribute("aria-pressed", "true");
});

test("selects both sides from the preview area outside the sheet", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const sheetSurface = await screen.findByLabelText(
    "Prévia do formato da lâmina",
  );
  const both = screen.getByRole("button", { name: "Ambos os lados" });
  expect(
    within(sheetSurface).queryByRole("button", { name: "Ambos os lados" }),
  ).not.toBeInTheDocument();
  const sideControls = within(sheetSurface).getByRole("group", {
    name: "Aplicar personalização em",
  });
  expect(within(sideControls).getAllByRole("button")).toHaveLength(2);
  expect(
    within(sideControls).getByRole("button", { name: "Página esquerda" }),
  ).toBeVisible();
  expect(
    within(sideControls).getByRole("button", { name: "Página direita" }),
  ).toBeVisible();

  await user.click(screen.getByRole("button", { name: "Página esquerda" }));
  expect(both).toHaveAttribute("aria-pressed", "false");
  // The real browser focus ring is covered by the UI acceptance scenario.
  const matches = both.matches.bind(both);
  const focusVisible = vi.spyOn(both, "matches").mockImplementation(selector => selector === ":focus-visible" || matches(selector));
  both.focus();
  await waitFor(() =>
    expect(
      screen.getByLabelText("Foco de teclado de ambos os lados"),
    ).toHaveAttribute("stroke", "#73A9CE"),
  );
  expect(screen.getByLabelText("Quadro de exemplo 1, lado direito")).toHaveAttribute(
    "fill-opacity",
    "0.15",
  );
  both.blur();
  await waitFor(() =>
    expect(
      screen.queryByLabelText("Foco de teclado de ambos os lados"),
    ).not.toBeInTheDocument(),
  );
  await user.click(both);
  expect(both).toHaveAttribute("aria-pressed", "true");
  focusVisible.mockRestore();
});

test("selects both sides from the preview legends outside the sheet", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const both = await screen.findByRole("button", { name: "Ambos os lados" });
  const left = screen.getByRole("button", { name: "Página esquerda" });

  await user.click(left);
  expect(both).toHaveAttribute("aria-pressed", "false");
  await user.click(screen.getByText("Lâmina aberta"));
  expect(both).toHaveAttribute("aria-pressed", "true");

  await user.click(left);
  expect(both).toHaveAttribute("aria-pressed", "false");
  await user.click(screen.getByText("Proporção real da lâmina aberta."));
  expect(both).toHaveAttribute("aria-pressed", "true");

  await user.click(left);
  expect(both).toHaveAttribute("aria-pressed", "false");
  await user.click(screen.getByLabelText("Prévia do formato da lâmina"));
  expect(both).toHaveAttribute("aria-pressed", "false");
});

test("hover previews the candidate without changing which side receives edits", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const both = await screen.findByRole("button", {
    name: "Ambos os lados",
  });
  const left = screen.getByRole("button", { name: "Página esquerda" });
  const right = screen.getByRole("button", { name: "Página direita" });
  expect(both).toHaveAttribute("aria-pressed", "true");
  expect(screen.queryAllByLabelText(/Atenuação do lado/)).toHaveLength(0);

  fireEvent.pointerEnter(left);
  expect(left).not.toHaveAttribute("data-highlighted");
  expect(both).toHaveAttribute("aria-pressed", "true");
  expect(
    screen.getByLabelText("Pré-seleção do lado esquerdo"),
  ).toHaveAttribute("data-scope", "left");
  expect(document.querySelector(".visual-scope-preview__selection")).toHaveAttribute("data-scope", "both");
  expect(screen.getByLabelText("Quadro de exemplo 1, lado esquerdo")).toHaveAttribute(
    "fill-opacity",
    "0.24",
  );
  expect(screen.getByLabelText("Quadro de exemplo 1, lado direito")).toHaveAttribute(
    "fill-opacity",
    "0.24",
  );
  chooseColor("do fundo", "#123456");
  expect(screen.getByLabelText("Fundo de ambos os lados")).toHaveAttribute(
    "fill",
    "#123456",
  );

  fireEvent.pointerLeave(left);
  expect(
    screen.queryByLabelText("Pré-seleção do lado esquerdo"),
  ).not.toBeInTheDocument();
  await user.click(left);
  expect(left).toHaveAttribute("aria-pressed", "true");
  fireEvent.pointerEnter(right);
  expect(screen.getByLabelText("Pré-seleção do lado direito")).toHaveAttribute(
    "data-scope",
    "right",
  );
  chooseColor("do fundo", "#abcdef");
  expect(screen.getByLabelText("Fundo do lado esquerdo")).toHaveAttribute(
    "fill",
    "#ABCDEF",
  );
  expect(screen.getByLabelText("Fundo do lado direito")).toHaveAttribute(
    "fill",
    "#123456",
  );

  await user.click(screen.getByRole("button", { name: "Criar projeto" }));
  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      visualDefaults: expect.objectContaining({
        background: {
          scope: "perSide",
          left: { kind: "color", rgb: "#ABCDEF" },
          right: { kind: "color", rgb: "#123456" },
        },
      }),
    }),
  );
});

test("presents divergent side values as mixed when returning to both sides", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  await user.click(
    await screen.findByRole("button", { name: "Página esquerda" }),
  );
  chooseColor("do fundo", "#abcdef");
  await user.click(screen.getByRole("button", { name: "Página direita" }));
  chooseColor("do fundo", "#123456");
  await user.click(screen.getByRole("button", { name: "Ambos os lados" }));

  const backgroundSection = screen
    .getByRole("heading", { name: "Fundo" })
    .closest("section") as HTMLElement;
  expect(within(backgroundSection).getByRole("button", { name: "Cor do fundo" })).toHaveAttribute("title", "Valores diferentes");
  expect(
    within(backgroundSection)
      .getAllByRole("button", { name: /Usar fundo/ })
      .some((button) => button.getAttribute("aria-pressed") === "true"),
  ).toBe(false);

  chooseColor("do fundo", "#eeeeee");
  expect(
    within(backgroundSection).queryByText("Valores diferentes"),
  ).not.toBeInTheDocument();
  expect(screen.getByLabelText("Fundo de ambos os lados")).toHaveAttribute(
    "fill",
    "#EEEEEE",
  );
});

test("page hover keeps the fixed selection and Frame contrast independent", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const left = await screen.findByRole("button", { name: "Página esquerda" });
  const right = screen.getByRole("button", { name: "Página direita" });
  await user.click(right);
  expect(
    screen.queryByLabelText("Foco de teclado do lado direito"),
  ).not.toBeInTheDocument();

  expect(screen.getByLabelText("Lado não selecionado: esquerdo")).toHaveAttribute(
    "fill",
    "#E3E0DA",
  );
  expect(screen.getByLabelText("Lado não selecionado: esquerdo")).toHaveAttribute(
    "fill-opacity",
    "0.42",
  );
  expect(screen.getByLabelText("Lado não selecionado: esquerdo")).toHaveAttribute(
    "stroke",
    "none",
  );
  expect(screen.getByLabelText("Lado não selecionado: esquerdo")).toHaveAttribute(
    "width",
    "300000",
  );
  fireEvent.pointerEnter(left);

  expect(right).toHaveAttribute("aria-pressed", "true");
  const fixedSelection = document.querySelector(
    ".visual-scope-preview__selection",
  );
  expect(fixedSelection).toHaveAttribute("aria-hidden", "true");
  expect(fixedSelection).toHaveAttribute("data-scope", "right");
  expect(screen.getByLabelText("Quadro de exemplo 1, lado direito")).toHaveAttribute(
    "fill-opacity",
    "0.24",
  );
  expect(screen.getByLabelText("Quadro de exemplo 1, lado esquerdo")).toHaveAttribute(
    "fill-opacity",
    "0.08",
  );
  expect(screen.getByLabelText("Pré-seleção do lado esquerdo")).toHaveAttribute(
    "data-scope",
    "left",
  );
  expect(screen.getByLabelText("Lado não selecionado: esquerdo")).toHaveAttribute(
    "fill-opacity",
    "0.18",
  );
});

test("uses the sheet outline as the keyboard focus indicator", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const left = await screen.findByRole("button", { name: "Página esquerda" });
  const right = await screen.findByRole("button", { name: "Página direita" });
  // jsdom does not model the browser's keyboard focus-visible modality.
  const matches = right.matches.bind(right);
  const focusVisible = vi.spyOn(right, "matches").mockImplementation(selector => selector === ":focus-visible" || matches(selector));
  left.focus();
  await user.tab();
  expect(right).toHaveFocus();
  await waitFor(() =>
    expect(
      screen.getByLabelText("Foco de teclado do lado direito"),
    ).toHaveAttribute("stroke", "#73A9CE"),
  );
  fireEvent.pointerEnter(left);
  expect(left).not.toHaveAttribute("data-highlighted");
  expect(
    screen.getByLabelText("Pré-seleção do lado esquerdo"),
  ).toHaveAttribute("data-scope", "left");
  expect(
    screen.getByLabelText("Foco de teclado do lado direito"),
  ).toHaveAttribute("fill", "none");
  expect(screen.getByLabelText("Foco de teclado do lado direito")).toHaveAttribute(
    "stroke",
    "#73A9CE",
  );
  expect(document.querySelector(".visual-scope-preview__selection")).toHaveAttribute("data-scope", "both");
  focusVisible.mockRestore();
});

test("shows a solid Frame border immediately and sends its canonical values", async () => {
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={validConfiguration}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "Continuar" }));
  fireEvent.change(
    await screen.findByRole("slider", {
      name: "Espessura da borda padrão",
    }),
    {
      target: { value: "2500" },
    },
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Usar cor da borda #C5A46D" }),
  );

  expect(screen.getByRole("spinbutton", { name: "Espessura da borda padrão em mm" })).toHaveValue("2.5");
  expect(
    screen.getByRole("button", { name: "Usar cor da borda #C5A46D" }),
  ).toHaveAttribute("aria-pressed", "true");

  const frameBorders = screen.getAllByLabelText(
    /Borda do quadro (esquerdo|direito) [12]/,
  );
  expect(frameBorders).toHaveLength(4);
  for (const frameBorder of frameBorders) {
    const segments = frameBorder.querySelectorAll("rect");
    expect(segments).toHaveLength(4);
    for (const segment of segments) {
      expect(segment).toHaveAttribute("fill", "#C5A46D");
    }
  }
  const firstFrameSegments = screen
    .getByLabelText("Borda do quadro esquerdo 1")
    .querySelectorAll("rect");
  expect(firstFrameSegments[0]).toHaveAttribute("x", "12000");
  expect(firstFrameSegments[0]).toHaveAttribute("y", "12000");
  expect(firstFrameSegments[0]).toHaveAttribute("width", "135500");
  expect(firstFrameSegments[0]).toHaveAttribute("height", "2500");
  expect(firstFrameSegments[3]).toHaveAttribute("x", "145000");
  expect(firstFrameSegments[3]).toHaveAttribute("width", "2500");

  fireEvent.click(screen.getByRole("button", { name: "Criar projeto" }));
  await waitFor(() =>
    expect(onCreate).toHaveBeenCalledWith(
      expect.objectContaining({
        visualDefaults: expect.objectContaining({
          frameBorder: {
            kind: "solid",
            rgb: "#C5A46D",
            widthUm: 2500,
          },
        }),
      }),
    ),
  );
});

test("restores the chosen Frame border color after passing through zero", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  const width = await screen.findByRole("slider", {
    name: "Espessura da borda padrão",
  });
  fireEvent.change(width, { target: { value: "2500" } });
  await user.click(
    screen.getByRole("button", { name: "Usar cor da borda #C5A46D" }),
  );

  fireEvent.change(width, { target: { value: "0" } });
  expect(screen.getByRole("slider", { name: "Espessura da borda padrão" })).toHaveAttribute("aria-valuetext", "sem borda");
  fireEvent.change(width, { target: { value: "1250" } });

  expect(
    screen.getByRole("button", { name: "Usar cor da borda #C5A46D" }),
  ).toHaveAttribute("aria-pressed", "true");
  for (const segment of screen
    .getByLabelText("Borda do quadro esquerdo 1")
    .querySelectorAll("rect")) {
    expect(segment).toHaveAttribute("fill", "#C5A46D");
  }

  await user.click(screen.getByRole("button", { name: "Criar projeto" }));
  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      visualDefaults: expect.objectContaining({
        frameBorder: {
          kind: "solid",
          rgb: "#C5A46D",
          widthUm: 1_250,
        },
      }),
    }),
  );
});

test("keeps distinct provisional images by side and sends only their opaque ids", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn<
    (
      configuration: NewProjectCreationConfiguration,
    ) => Promise<ProjectLaunchOutcome>
  >(async () => ({ status: "cancelled" }));
  const chooseDecorative = vi
    .fn()
    .mockResolvedValueOnce(selectedDecorative({
      selectionId: "selection-background-left",
      displayName: "Fundo esquerdo.jpg",
      previewUrl: "blob:background-left",
    }))
    .mockResolvedValueOnce(selectedDecorative({
      selectionId: "selection-background-right",
      displayName: "Fundo direito.jpg",
      previewUrl: "blob:background-right",
    }))
    .mockResolvedValueOnce(selectedDecorative({
      selectionId: "selection-overlay-right",
      displayName: "Sobreposição direito.png",
      previewUrl: "blob:overlay-right",
    }));

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onChooseDecorative={chooseDecorative}
      onCreate={onCreate}
      onReleaseDecorative={vi.fn()}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  await user.click(
    await screen.findByRole("button", { name: "Página esquerda" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de fundo…" }),
  );
  expect(await screen.findByText("Fundo esquerdo.jpg")).toBeInTheDocument();
  const leftBackground = screen.getByLabelText("Fundo do lado esquerdo");
  expect(leftBackground).toHaveAttribute(
    "href",
    "blob:background-left",
  );
  const canonicalBase = screen.getByLabelText("Fundo branco");
  expect(canonicalBase.parentElement).toBe(leftBackground.parentElement);
  expect(
    [...(canonicalBase.parentElement?.children ?? [])].indexOf(canonicalBase),
  ).toBeLessThan(
    [...(leftBackground.parentElement?.children ?? [])].indexOf(leftBackground),
  );

  await user.click(screen.getByRole("button", { name: "Página direita" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de fundo…" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem… de sobreposição" }),
  );
  expect(await screen.findByText("Sobreposição direito.png")).toBeInTheDocument();
  expect(screen.getByLabelText("Sobreposição do lado direito")).toHaveAttribute(
    "href",
    "blob:overlay-right",
  );

  await user.click(screen.getByRole("button", { name: "Criar projeto" }));
  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      visualDefaults: {
        background: {
          scope: "perSide",
          left: { kind: "image", selectionId: "selection-background-left" },
          right: {
            kind: "image",
            selectionId: "selection-background-right",
          },
        },
        overlay: {
          scope: "perSide",
          left: null,
          right: { kind: "image", selectionId: "selection-overlay-right" },
        },
        frameBorder: { kind: "none" },
      },
    }),
  );
  expect(JSON.stringify(onCreate.mock.calls[0]?.[0])).not.toContain(
    "blob:background-left",
  );
  expect(JSON.stringify(onCreate.mock.calls[0]?.[0])).not.toContain(
    "Fundo esquerdo.jpg",
  );
});

test("preserves provisional personalization and releases it when creation is cancelled", async () => {
  const user = userEvent.setup();
  const onCancel = vi.fn();
  const onReleaseDecorative = vi.fn();
  const onOperationalFailure = vi.fn(async () => undefined);
  const selection = {
    selectionId: "selection-kept",
    displayName: "Fundo preservado.jpg",
    previewUrl: "blob:background-kept",
  };
  const onChooseDecorative = vi
    .fn()
    .mockResolvedValueOnce(selectedDecorative(selection))
    .mockResolvedValueOnce({ status: "cancelled" });

  render(
    <NewProjectFlow
      onCancel={onCancel}
      onChooseDecorative={onChooseDecorative}
      onCreate={async () => ({ status: "cancelled" })}
      onOperationalFailure={onOperationalFailure}
      onReleaseDecorative={onReleaseDecorative}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de fundo…" }),
  );
  expect(await screen.findByText(selection.displayName)).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Criar projeto" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de fundo…" }),
  );
  expect(screen.getByText(selection.displayName)).toBeInTheDocument();
  expect(onReleaseDecorative).not.toHaveBeenCalled();
  expect(onOperationalFailure).not.toHaveBeenCalled();

  await user.click(screen.getByRole("button", { name: "Voltar" }));
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(await screen.findByText(selection.displayName)).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(onReleaseDecorative).toHaveBeenCalledOnce();
  expect(onReleaseDecorative).toHaveBeenCalledWith(selection.selectionId);
  expect(onCancel).toHaveBeenCalledOnce();
});

test("applies a reusable preset across both creation steps", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  await user.selectOptions(
    screen.getByRole("combobox", { name: "Modelo inicial" }),
    "builtin-graphic-30",
  );
  expect(
    screen.getByRole("textbox", { name: "Largura" }),
  ).toHaveValue("300");
  expect(
    screen.getByRole("textbox", { name: "Lâminas" }),
  ).toHaveValue("18");
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("5");

  await user.click(
    await screen.findByRole("button", { name: "Continuar" }),
  );
  expect(
    await screen.findByRole("button", {
      name: "Usar fundo #f7f5f0",
    }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("keeps a custom preset across both steps for the current placeholder session", async () => {
  const user = userEvent.setup();
  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  fireEvent.click(
    await screen.findByRole("button", {
      name: "Usar fundo #1d2a3a",
    }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Voltar" }));
  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura" }),
    { target: { value: "320" } },
  );

  fireEvent.click(
    screen.getByRole("button", {
      name: "Salvar configuração atual como modelo",
    }),
  );
  const saveModelForm = screen.getByRole("dialog", { name: "Salvar modelo" });
  expect(
    within(saveModelForm).getByRole("button", { name: "Cancelar" }),
  ).toHaveClass("ui-action-button");
  expect(
    within(saveModelForm).getByRole("button", { name: "Salvar" }),
  ).toHaveClass("ui-action-button", "ui-action-button--primary");
  fireEvent.change(screen.getByRole("textbox", { name: "Nome do modelo" }), {
    target: { value: "Estúdio 32 × 30" },
  });
  fireEvent.click(screen.getByRole("button", { name: "Salvar" }));
  expect(
    screen.getByRole("combobox", { name: "Modelo inicial" }),
  ).toHaveDisplayValue("Estúdio 32 × 30");

  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura" }),
    { target: { value: "300" } },
  );
  fireEvent.change(screen.getByRole("combobox", { name: "Modelo inicial" }), {
    target: { value: "custom-1" },
  });
  expect(
    screen.getByRole("textbox", { name: "Largura" }),
  ).toHaveValue("320");

  await user.click(
    await screen.findByRole("button", { name: "Continuar" }),
  );
  expect(
    await screen.findByRole("button", {
      name: "Usar fundo #1d2a3a",
    }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("dismisses the Save model popover and restores focus to its trigger", async () => {
  const user = userEvent.setup();
  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );

  const trigger = screen.getByRole("button", {
    name: "Salvar configuração atual como modelo",
  });
  expect(trigger).toHaveAttribute("aria-expanded", "false");

  await user.click(trigger);
  const form = screen.getByRole("dialog", { name: "Salvar modelo" });
  expect(trigger).toHaveAttribute("aria-expanded", "true");
  expect(trigger).toHaveAttribute("aria-controls", form.id);
  expect(screen.getByRole("textbox", { name: "Nome do modelo" })).toHaveFocus();

  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog", { name: "Salvar modelo" })).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();

  await user.click(trigger);
  fireEvent.pointerDown(screen.getByRole("button", { name: "Continuar" }));
  expect(screen.queryByRole("dialog", { name: "Salvar modelo" })).not.toBeInTheDocument();
  expect(trigger).toHaveFocus();

  await user.click(trigger);
  await user.click(
    within(screen.getByRole("dialog", { name: "Salvar modelo" })).getByRole(
      "button",
      { name: "Cancelar" },
    ),
  );
  expect(trigger).toHaveFocus();

  await user.click(trigger);
  await user.type(
    screen.getByRole("textbox", { name: "Nome do modelo" }),
    "Modelo com foco",
  );
  await user.click(
    within(screen.getByRole("dialog", { name: "Salvar modelo" })).getByRole(
      "button",
      { name: "Salvar" },
    ),
  );
  expect(trigger).toHaveFocus();
});

test("shows a typed native picker failure without changing personalization", async () => {
  const user = userEvent.setup();
  const error = {
    code: "unsupported_image",
    message: "O arquivo escolhido não contém uma imagem JPEG ou PNG.",
    action: "Escolha outro arquivo JPEG ou PNG.",
  };
  const onOperationalFailure = vi.fn(async () => undefined);

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onChooseDecorative={vi.fn(async () => ({
        status: "failed" as const,
        error,
      }))}
      onCreate={async () => ({ status: "cancelled" })}
      onOperationalFailure={onOperationalFailure}
      onReleaseDecorative={vi.fn()}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de fundo…" }),
  );

  await waitFor(() =>
    expect(onOperationalFailure).toHaveBeenCalledWith({
      context: "decorativeSelection",
      error,
    }),
  );
  expect(onOperationalFailure).toHaveBeenCalledOnce();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Cor do fundo" })).toBeInTheDocument();
});

test("releases a provisional image as soon as it is no longer referenced", async () => {
  const user = userEvent.setup();
  const firstSelection = {
    selectionId: "selection-replaced",
    displayName: "Primeiro fundo.jpg",
    previewUrl: "blob:first-background",
  };
  const secondSelection = {
    selectionId: "selection-current",
    displayName: "Fundo atual.jpg",
    previewUrl: "blob:current-background",
  };
  const onReleaseDecorative = vi.fn();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onChooseDecorative={vi
        .fn()
        .mockResolvedValueOnce(selectedDecorative(firstSelection))
        .mockResolvedValueOnce(selectedDecorative(secondSelection))}
      onCreate={async () => ({ status: "cancelled" })}
      onReleaseDecorative={onReleaseDecorative}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  const chooseBackground = await screen.findByRole("button", {
    name: "Escolher imagem de fundo…",
  });
  await user.click(chooseBackground);
  expect(await screen.findByText(firstSelection.displayName)).toBeInTheDocument();
  await user.click(chooseBackground);

  expect(await screen.findByText(secondSelection.displayName)).toBeInTheDocument();
  expect(onReleaseDecorative).toHaveBeenCalledOnce();
  expect(onReleaseDecorative).toHaveBeenCalledWith(
    firstSelection.selectionId,
  );
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

  const width = screen.getByRole("textbox", {
    name: "Largura",
  });
  const height = screen.getByRole("textbox", {
    name: "Altura",
  });
  await user.click(screen.getByRole("button", { name: "pol" }));
  expect(width).toHaveValue("11.811");
  expect(height).toHaveValue("11.811");
  expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue(
    "0.118",
  );
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("0.118");
  expect(width.closest(".ui-text-field__entry")).toHaveTextContent("pol");
  expect(width.closest(".ui-text-field__entry")).not.toHaveTextContent(
    "in",
  );
  fireEvent.change(width, { target: { value: "11.81" } });
  fireEvent.change(width, { target: { value: "11.811" } });
  expect(
    screen.queryByText("Informe uma medida válida em pol."),
  ).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "mm" }));
  expect(width).toHaveValue("300");
  expect(height).toHaveValue("300");

  await user.click(screen.getByRole("button", { name: "cm" }));
  fireEvent.change(width, { target: { value: "25.4" } });
  fireEvent.change(height, { target: { value: "25.4" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Resolução" }), {
    target: { value: "600" },
  });
  fireEvent.change(
    screen.getByRole("textbox", { name: "Lâminas" }),
    { target: { value: "4" } },
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Primeira lâmina" }),
    "singlePage",
  );
  fireEvent.change(screen.getByRole("textbox", { name: "Sangria" }), {
    target: { value: "0" },
  });

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(
    await screen.findByLabelText("Prévia do formato da lâmina"),
  ).toHaveStyle({ aspectRatio: "508000 / 254000" });
  await user.click(screen.getByRole("button", { name: "Criar projeto" }));
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
    visualDefaults: neutralVisualDefaults,
    frameGapUm: 5_000,
  } satisfies NewProjectCreationConfiguration);
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
    name: "Largura",
  });
  fireEvent.change(width, { target: { value: "60.0001" } });
  expect(screen.queryByText(/medida válida em mm/i)).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(width).toHaveFocus();
  expect(screen.getByRole("alert")).toHaveTextContent(/medida válida em mm/i);
  expect(onValidate).not.toHaveBeenCalled();

  fireEvent.change(width, { target: { value: "600" } });
  await waitFor(() => expect(onValidate).toHaveBeenCalledOnce());
  await waitFor(() =>
    expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
  );
  expect(
    screen.getByRole("heading", { name: "Configurações" }),
  ).toBeInTheDocument();
});

test("shows every Core error, focuses the first field and refreshes errors after editing", async () => {
  const user = userEvent.setup();
  const onValidate = vi
    .fn<() => Promise<ProjectConfigurationValidationOutcome>>()
    .mockResolvedValueOnce({ rasterLimits: rasterLimitsAt300Dpi,
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
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent(/altura.*maior que zero/i);
  expect(alert).toHaveTextContent(/número inteiro entre 1 e 1\.200 DPI/i);
  expect(alert).toHaveTextContent(/pelo menos 2 lâminas/i);
  const height = screen.getByRole("textbox", {
    name: "Altura",
  });
  const descriptionId = height.getAttribute("aria-describedby");
  expect(descriptionId).toBe(alert.id);
  expect(screen.getByRole("textbox", { name: "Resolução" })).toHaveAttribute(
    "aria-describedby",
    descriptionId,
  );
  expect(
    screen.getByRole("textbox", { name: "Lâminas" }),
  ).toHaveAttribute("aria-describedby", descriptionId);
  expect(height).not.toHaveAttribute("title");
  expect(screen.getAllByRole("alert")).toHaveLength(1);
  expect(screen.getByRole("tooltip")).toHaveTextContent(
    "A altura da lâmina deve ser maior que zero.",
  );
  // Resolução leads the panel, so it is the first invalid field in visual order.
  expect(screen.getByRole("textbox", { name: "Resolução" })).toHaveFocus();

  fireEvent.pointerDown(
    screen.getByRole("heading", { name: "Configurações" }),
  );
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();
  expect(height).toHaveAttribute("aria-invalid", "true");

  fireEvent.focus(screen.getByRole("textbox", { name: "Resolução" }));
  expect(screen.getByRole("tooltip")).toHaveTextContent(
    "Use um número inteiro entre 1 e 1.200 DPI.",
  );
  fireEvent.pointerDown(
    screen.getByRole("heading", { name: "Configurações" }),
  );
  expect(screen.queryByRole("tooltip")).not.toBeInTheDocument();

  fireEvent.change(
    screen.getByRole("textbox", { name: "Altura" }),
    { target: { value: "250" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(screen.queryByRole("alert")).not.toBeInTheDocument(),
  );
  expect(
    screen.getByRole("heading", { name: "Configurações" }),
  ).toBeInTheDocument();
});

test("presents creation raster limits in the selected Unit and closed Sheet measurement", async () => {
  const user = userEvent.setup();
  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={vi.fn(async () => ({ rasterLimits: rasterLimitsAt300Dpi,
        status: "invalid" as const,
        errors: ["sheetWidthRasterOutOfRange" as const],
      }))}
    />,
  );

  await user.click(screen.getByRole("button", { name: "cm" }));
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const tooltip = await screen.findByRole("tooltip");
  expect(tooltip).toHaveTextContent(
    "Para 300 DPI, informe a largura da lâmina fechada entre 0.0043 cm e 277.4336 cm.",
  );
  expect(tooltip).not.toHaveTextContent(/pixels?/i);
});

test("anchors validation to the first invalid field in visual order", async () => {
  const user = userEvent.setup();
  const onValidate = vi
    .fn<() => Promise<ProjectConfigurationValidationOutcome>>()
    .mockResolvedValue({ rasterLimits: rasterLimitsAt300Dpi,
      status: "invalid",
      errors: ["dpiOutOfRange", "bleedNegative"],
    });

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={onValidate}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  // Documento (Resolução) comes before Áreas técnicas (Sangria) in the panel.
  const resolution = screen.getByRole("textbox", { name: "Resolução" });
  const tooltip = await screen.findByRole("tooltip");
  expect(resolution).toHaveFocus();
  expect(tooltip).toHaveTextContent("A sangria não pode ser negativa.");
  const summary = (tooltip.textContent ?? "").toLowerCase();
  expect(summary.indexOf("dpi")).toBeGreaterThanOrEqual(0);
  expect(summary.indexOf("dpi")).toBeLessThan(summary.indexOf("sangria"));
});

test("preserves errors from untouched fields during live validation", async () => {
  const user = userEvent.setup();
  const liveValidation = deferred<ProjectConfigurationValidationOutcome>();
  const onValidate = vi
    .fn<() => Promise<ProjectConfigurationValidationOutcome>>()
    .mockResolvedValueOnce({ rasterLimits: rasterLimitsAt300Dpi,
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
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const alert = await screen.findByRole("alert");
  expect(alert).toHaveTextContent(/número inteiro entre 1 e 1\.200 DPI/i);
  expect(alert).toHaveTextContent(/pelo menos 2 lâminas/i);

  fireEvent.change(
    screen.getByRole("textbox", { name: "Altura" }),
    { target: { value: "250" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));
  expect(screen.getByRole("alert")).toHaveTextContent(
    /número inteiro entre 1 e 1\.200 DPI/i,
  );
  expect(screen.getByRole("alert")).toHaveTextContent(/pelo menos 2 lâminas/i);

  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura" }),
    { target: { value: "600.0001" } },
  );
  expect(screen.getByRole("alert")).toHaveTextContent(/medida válida em mm/i);
  expect(screen.getByRole("alert")).toHaveTextContent(
    /número inteiro entre 1 e 1\.200 DPI/i,
  );
  expect(screen.getByRole("alert")).toHaveTextContent(/pelo menos 2 lâminas/i);
  expect(onValidate).toHaveBeenCalledTimes(2);

  await act(async () => {
    liveValidation.resolve({ status: "valid" });
  });
  expect(screen.getByRole("alert")).toHaveTextContent(/medida válida em mm/i);
  expect(screen.getByRole("alert")).toHaveTextContent(
    /número inteiro entre 1 e 1\.200 DPI/i,
  );
  expect(screen.getByRole("alert")).toHaveTextContent(/pelo menos 2 lâminas/i);
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
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await waitFor(() => expect(onValidate).toHaveBeenCalledOnce());
  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura" }),
    { target: { value: "500" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));

  await act(async () => {
    second.resolve({ rasterLimits: rasterLimitsAt300Dpi,
      status: "invalid",
      errors: ["sheetWidthNotEven"],
    });
  });
  expect(screen.getByRole("alert")).toHaveTextContent(
    /duas páginas com a mesma medida/i,
  );

  await act(async () => {
    first.resolve({ status: "valid" });
  });
  expect(screen.getByRole("alert")).toHaveTextContent(
    /duas páginas com a mesma medida/i,
  );
  expect(
    screen.getByRole("heading", { name: "Configurações" }),
  ).toBeInTheDocument();
});

test("blocks navigation and shows an actionable operational validation failure", async () => {
  const user = userEvent.setup();
  const onCreate = vi.fn(async () => ({ status: "cancelled" as const }));
  const error = {
    code: "validation_unavailable",
    message: "A validação está indisponível.",
    action: "Tente novamente.",
  };
  const onOperationalFailure = vi.fn(async () => undefined);

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onOperationalFailure={onOperationalFailure}
      onValidate={async () => ({
        status: "failed",
        error,
      })}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  await waitFor(() =>
    expect(onOperationalFailure).toHaveBeenCalledWith({
      context: "configurationValidation",
      error,
    }),
  );
  expect(onOperationalFailure).toHaveBeenCalledOnce();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Configurações" }),
  ).toBeInTheDocument();
  expect(onCreate).not.toHaveBeenCalled();
});

test("cancels before validation or creation", async () => {
  const user = userEvent.setup();
  const onCancel = vi.fn();
  const onValidate = vi.fn(validConfiguration);
  const onCreate = vi.fn(async () => ({ status: "opened" as const }));
  const onOperationalFailure = vi.fn(async () => undefined);

  render(
    <NewProjectFlow
      onCancel={onCancel}
      onCreate={onCreate}
      onOperationalFailure={onOperationalFailure}
      onValidate={onValidate}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(onCancel).toHaveBeenCalledOnce();
  expect(onValidate).not.toHaveBeenCalled();
  expect(onCreate).not.toHaveBeenCalled();
  expect(onOperationalFailure).not.toHaveBeenCalled();
});

test("preserves the draft after native cancellation and structured creation failure", async () => {
  const user = userEvent.setup();
  const nativeCreation = deferred<ProjectLaunchOutcome>();
  const error = {
    code: "destination_conflict",
    message: "Outro objeto passou a ocupar este destino.",
  };
  const onOperationalFailure = vi.fn(async () => undefined);
  const onCreate = vi
    .fn<() => Promise<ProjectLaunchOutcome>>()
    .mockReturnValueOnce(nativeCreation.promise)
    .mockResolvedValueOnce({
      status: "failed",
      error,
    });

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={onCreate}
      onOperationalFailure={onOperationalFailure}
      onValidate={validConfiguration}
    />,
  );
  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura" }),
    { target: { value: "500" } },
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(
    await screen.findByRole("button", { name: "Criar projeto" }),
  );
  await act(async () => nativeCreation.resolve({ status: "cancelled" }));
  expect(onOperationalFailure).not.toHaveBeenCalled();

  await user.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByRole("textbox", { name: "Largura" }),
  ).toHaveValue("500");
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(
    await screen.findByRole("button", { name: "Criar projeto" }),
  );
  await waitFor(() =>
    expect(onOperationalFailure).toHaveBeenCalledWith({
      context: "projectCreation",
      error,
    }),
  );
  expect(onOperationalFailure).toHaveBeenCalledOnce();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByRole("textbox", { name: "Largura" }),
  ).toHaveValue("500");
});
