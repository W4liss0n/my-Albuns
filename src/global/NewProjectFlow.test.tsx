import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";

import type {
  NewProjectConfiguration,
  ProjectConfigurationValidationOutcome,
  ProjectLaunchOutcome,
  ProvisionalDecorativeSelection,
  ProvisionalDecorativeSelectionOutcome,
} from "./application/globalProjectPort";
import type { NewProjectCreationConfiguration } from "./application/newProjectPersonalization";
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
    screen.getByRole("region", { name: "Prévia da Lâmina aberta" });

  expect(within(previewPanel()).getByText("Lâmina aberta")).toBeVisible();
  expect(within(previewPanel()).getByText(/18 Lâminas/)).toBeVisible();
  expect(
    within(previewPanel()).getByText("Proporção real da Lâmina aberta."),
  ).toBeVisible();
  expect(
    within(previewPanel()).getByLabelText("Guias técnicas da Lâmina"),
  ).toBeInTheDocument();
  expect(
    within(previewPanel()).getByRole("img", { name: "Prévia das Dimensões" }),
  ).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Continuar" }));

  expect(within(previewPanel()).getByText("Lâmina aberta")).toBeVisible();
  expect(within(previewPanel()).getByText(/18 Lâminas/)).toBeVisible();
  expect(
    within(previewPanel()).getByText("Proporção real da Lâmina aberta."),
  ).toBeVisible();
  expect(
    within(previewPanel()).getByLabelText("Guias técnicas da Lâmina"),
  ).toBeInTheDocument();
  expect(
    within(previewPanel()).getByRole("img", {
      name: "Reprodução da Lâmina",
    }),
  ).toBeInTheDocument();
  expect(
    within(previewPanel()).queryByRole("img", {
      name: "Prévia das Dimensões",
    }),
  ).not.toBeInTheDocument();
});

test("shares one header between the steps and the preset control", async () => {
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
  expect(header).toContainElement(
    screen.getByRole("combobox", { name: "Modelo inicial" }),
  );

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(
    await screen.findByRole("combobox", { name: "Modelo inicial" }),
  ).toBeVisible();
  expect(header).toContainElement(
    screen.getByRole("combobox", { name: "Modelo inicial" }),
  );
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
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
  ).toHaveValue("300");
  expect(
    screen.getByRole("textbox", { name: "Altura da Lâmina fechada" }),
  ).toHaveValue("300");
  expect(screen.getByRole("textbox", { name: "DPI" })).toHaveValue("300");
  expect(
    screen.getByRole("textbox", { name: "Quantidade de Lâminas" }),
  ).toHaveValue("18");
  expect(
    screen
      .getByRole("combobox", { name: "Modelo inicial" })
      .closest("section"),
  ).toHaveAttribute("data-placeholder-feature", "new-project-presets");
  await user.click(
    screen.getByRole("button", {
      name: "Aumentar quantidade de Lâminas",
    }),
  );
  expect(
    screen.getByRole("textbox", { name: "Quantidade de Lâminas" }),
  ).toHaveValue("20");
  await user.click(
    screen.getByRole("button", {
      name: "Diminuir quantidade de Lâminas",
    }),
  );
  expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue("3");
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("5");
  expect(
    screen.queryByText(
      "Lâmina 600 × 300 mm · Página 300 × 300 mm · 300 DPI",
    ),
  ).not.toBeInTheDocument();
  expect(screen.queryByText("Configurações avançadas")).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Resolução do Projeto" }),
  ).toBeVisible();
  expect(
    screen.getByRole("heading", { name: "Configuração das extremidades" }),
  ).toBeVisible();
  expect(
    screen.queryByText(
      "Medidas, Sangria e Área de segurança valem para o Álbum inteiro e podem ser alteradas depois nas Configurações do Projeto.",
    ),
  ).not.toBeInTheDocument();
  expect(
    screen.queryByLabelText("Reprodução da Lâmina"),
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
      safetyUm: 5_000,
    },
    structure: {
      sheetCount: 18,
      firstSheet: "double",
      lastSheet: "double",
    },
  } satisfies NewProjectConfiguration;
  expect(onValidate).toHaveBeenCalledWith(expectedConfiguration);

  await user.click(screen.getByRole("button", { name: "Criar" }));
  expect(onCreate).toHaveBeenCalledWith({
    ...expectedConfiguration,
    visualDefaults: neutralVisualDefaults,
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
  await screen.findByRole("button", { name: "Criar" });

  expect(footerActionNames()).toEqual(["Cancelar", "Voltar", "Criar"]);
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
    await screen.findByRole("img", { name: "Reprodução da Lâmina" }),
  ).toBeInTheDocument();
  expect(screen.getByLabelText("Base branca canônica")).toHaveAttribute(
    "fill",
    "#FFFFFF",
  );
  const demonstrativeFrames = screen.getAllByLabelText(
    /Frame demonstrativo (esquerdo|direito) [12]/,
  );
  expect(demonstrativeFrames).toHaveLength(4);
  for (const frame of demonstrativeFrames) {
    expect(frame).toHaveAttribute("fill", "#7A684E");
    expect(frame).toHaveAttribute("fill-opacity", "0.24");
    expect(frame).toHaveAttribute("stroke", "none");
  }
  expect(screen.getByLabelText("Cor do Background")).toHaveValue("#ffffff");
  expect(
    screen.queryByRole("checkbox", { name: "Borda dos Frames" }),
  ).not.toBeInTheDocument();
  expect(
    screen.getByRole("slider", { name: "Espessura da Borda padrão" }),
  ).toHaveValue("0");
  expect(screen.getByText("sem borda")).toBeVisible();
  const borderColors = within(
    screen.getByRole("group", { name: "Cores da Borda" }),
  ).getAllByRole("button");
  expect(borderColors).toHaveLength(3);
  expect(borderColors[0]).toHaveAttribute("aria-pressed", "true");
  expect(
    screen.getByRole("slider", { name: "Espaço entre Frames" }),
  ).toHaveValue("6000");
  expect(screen.getByText("6 mm")).toBeVisible();
  expect(
    screen
      .getByRole("slider", { name: "Espaço entre Frames" })
      .closest("div"),
  ).toHaveAttribute("data-placeholder-feature", "new-project-frame-gap");
  const secondFrame = screen.getByLabelText("Frame demonstrativo esquerdo 2");
  const initialSecondFrameX = Number(secondFrame.getAttribute("x"));
  fireEvent.change(
    screen.getByRole("slider", { name: "Espaço entre Frames" }),
    { target: { value: "18000" } },
  );
  expect(
    screen.getByRole("slider", { name: "Espaço entre Frames" }),
  ).toHaveValue("18000");
  expect(screen.getByText("18 mm")).toBeVisible();
  expect(Number(secondFrame.getAttribute("x"))).toBeGreaterThan(
    initialSecondFrameX,
  );
  expect(screen.getByLabelText("Background de ambos os lados")).toHaveAttribute(
    "fill",
    "#FFFFFF",
  );

  await user.click(screen.getByRole("button", { name: "Criar" }));

  expect(onCreate).toHaveBeenCalledWith(
    expect.objectContaining({
      visualDefaults: neutralVisualDefaults,
    }),
  );
  expect(JSON.stringify(onCreate.mock.calls[0]?.[0])).not.toContain(
    '"frames"',
  );
  expect(JSON.stringify(onCreate.mock.calls[0]?.[0])).not.toContain(
    '"frameGap"',
  );
});

test("formats the placeholder Frame spacing in the configured Unit", async () => {
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
    screen.getByRole("slider", { name: "Espaço entre Frames" }),
  ).toHaveValue("6000");
  expect(screen.getByText("0.6 cm")).toBeVisible();
});

test("does not hover a side that already belongs to the fixed scope", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const left = await screen.findByRole("button", { name: "Lado esquerdo" });
  const right = screen.getByRole("button", { name: "Lado direito" });
  fireEvent.pointerEnter(left);
  expect(
    screen.queryByLabelText("Pré-seleção do lado esquerdo"),
  ).not.toBeInTheDocument();

  await user.click(left);
  fireEvent.pointerEnter(left);
  expect(
    screen.queryByLabelText("Pré-seleção do lado esquerdo"),
  ).not.toBeInTheDocument();

  fireEvent.pointerEnter(right);
  expect(screen.getByLabelText("Pré-seleção do lado direito")).toHaveAttribute(
    "fill",
    "var(--ui-text-muted)",
  );
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
    "Prévia do formato da Lâmina",
  );
  const both = screen.getByRole("button", { name: "Ambos os lados" });
  expect(
    within(sheetSurface).queryByRole("button", { name: "Ambos os lados" }),
  ).not.toBeInTheDocument();
  const sideControls = within(sheetSurface).getByRole("group", {
    name: "Escopo da personalização",
  });
  expect(within(sideControls).getAllByRole("button")).toHaveLength(2);
  expect(
    within(sideControls).getByRole("button", { name: "Lado esquerdo" }),
  ).toBeVisible();
  expect(
    within(sideControls).getByRole("button", { name: "Lado direito" }),
  ).toBeVisible();

  await user.click(screen.getByRole("button", { name: "Lado esquerdo" }));
  expect(both).toHaveAttribute("aria-pressed", "false");
  both.focus();
  await waitFor(() =>
    expect(
      screen.getByLabelText("Foco de teclado de ambos os lados"),
    ).toHaveAttribute("stroke", "#73A9CE"),
  );
  expect(screen.getByLabelText("Frame demonstrativo direito 1")).toHaveAttribute(
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
  const left = screen.getByRole("button", { name: "Lado esquerdo" });

  await user.click(left);
  expect(both).toHaveAttribute("aria-pressed", "false");
  await user.click(screen.getByText("Lâmina aberta"));
  expect(both).toHaveAttribute("aria-pressed", "true");

  await user.click(left);
  expect(both).toHaveAttribute("aria-pressed", "false");
  await user.click(screen.getByText("Proporção real da Lâmina aberta."));
  expect(both).toHaveAttribute("aria-pressed", "true");

  await user.click(left);
  expect(both).toHaveAttribute("aria-pressed", "false");
  await user.click(screen.getByLabelText("Prévia do formato da Lâmina"));
  expect(both).toHaveAttribute("aria-pressed", "false");
});

test("hover fills only an unselected candidate without changing the fixed scope", async () => {
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
  const left = screen.getByRole("button", { name: "Lado esquerdo" });
  const right = screen.getByRole("button", { name: "Lado direito" });
  expect(both).toHaveAttribute("aria-pressed", "true");
  expect(screen.queryAllByLabelText(/Atenuação do lado/)).toHaveLength(0);

  fireEvent.pointerEnter(left);
  expect(left).not.toHaveAttribute("data-highlighted");
  expect(both).toHaveAttribute("aria-pressed", "true");
  expect(
    screen.queryByLabelText("Pré-seleção do lado esquerdo"),
  ).not.toBeInTheDocument();
  expect(document.querySelector(".new-project-fixed-selection")).toHaveClass(
    "new-project-fixed-selection--both",
  );
  expect(screen.getByLabelText("Frame demonstrativo esquerdo 1")).toHaveAttribute(
    "fill-opacity",
    "0.24",
  );
  expect(screen.getByLabelText("Frame demonstrativo direito 1")).toHaveAttribute(
    "fill-opacity",
    "0.24",
  );
  fireEvent.change(screen.getByLabelText("Cor do Background"), {
    target: { value: "#123456" },
  });
  expect(screen.getByLabelText("Background de ambos os lados")).toHaveAttribute(
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
  const hoverFill = screen
    .getByLabelText("Pré-seleção do lado direito")
    .getAttribute("fill");
  expect(hoverFill).toBe("var(--ui-text-muted)");
  expect(screen.getByLabelText("Pré-seleção do lado direito")).toHaveAttribute(
    "fill-opacity",
    "0.08",
  );
  expect(screen.getByLabelText("Pré-seleção do lado direito")).toHaveAttribute(
    "stroke",
    "none",
  );
  fireEvent.change(screen.getByLabelText("Cor do Background"), {
    target: { value: "#abcdef" },
  });
  expect(screen.getByLabelText("Background do lado esquerdo")).toHaveAttribute(
    "fill",
    "#ABCDEF",
  );
  expect(screen.getByLabelText("Background do lado direito")).toHaveAttribute(
    "fill",
    "#123456",
  );

  await user.click(screen.getByRole("button", { name: "Criar" }));
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

test("hover tint keeps the fixed selection and Frame contrast independent", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onCreate={vi.fn(async () => ({ status: "cancelled" as const }))}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  const left = await screen.findByRole("button", { name: "Lado esquerdo" });
  const right = screen.getByRole("button", { name: "Lado direito" });
  await user.click(right);
  expect(
    screen.queryByLabelText("Foco de teclado do lado direito"),
  ).not.toBeInTheDocument();

  expect(screen.getByLabelText("Atenuação do lado esquerdo")).toHaveAttribute(
    "fill",
    "#E3E0DA",
  );
  expect(screen.getByLabelText("Atenuação do lado esquerdo")).toHaveAttribute(
    "fill-opacity",
    "0.42",
  );
  expect(screen.getByLabelText("Atenuação do lado esquerdo")).toHaveAttribute(
    "stroke",
    "none",
  );
  expect(screen.getByLabelText("Atenuação do lado esquerdo")).toHaveAttribute(
    "width",
    "300000",
  );
  fireEvent.pointerEnter(left);

  expect(right).toHaveAttribute("aria-pressed", "true");
  const fixedSelection = document.querySelector(
    ".new-project-fixed-selection",
  );
  expect(fixedSelection).toHaveAttribute("aria-hidden", "true");
  expect(fixedSelection).toHaveClass(
    "new-project-fixed-selection--right",
  );
  expect(screen.getByLabelText("Frame demonstrativo direito 1")).toHaveAttribute(
    "fill-opacity",
    "0.24",
  );
  expect(screen.getByLabelText("Frame demonstrativo esquerdo 1")).toHaveAttribute(
    "fill-opacity",
    "0.08",
  );
  expect(screen.getByLabelText("Pré-seleção do lado esquerdo")).toHaveAttribute(
    "fill-opacity",
    "0.08",
  );
  expect(screen.getByLabelText("Pré-seleção do lado esquerdo")).toHaveAttribute(
    "stroke",
    "none",
  );
  expect(screen.getByLabelText("Atenuação do lado esquerdo")).toHaveAttribute(
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

  const left = await screen.findByRole("button", { name: "Lado esquerdo" });
  const right = await screen.findByRole("button", { name: "Lado direito" });
  right.focus();
  expect(right).toHaveFocus();
  await waitFor(() =>
    expect(
      screen.getByLabelText("Foco de teclado do lado direito"),
    ).toHaveAttribute("stroke", "#73A9CE"),
  );
  fireEvent.pointerEnter(left);
  expect(left).not.toHaveAttribute("data-highlighted");
  expect(
    screen.queryByLabelText("Pré-seleção do lado esquerdo"),
  ).not.toBeInTheDocument();
  expect(
    screen.getByLabelText("Foco de teclado do lado direito"),
  ).toHaveAttribute("fill", "none");
  expect(screen.getByLabelText("Foco de teclado do lado direito")).toHaveAttribute(
    "stroke",
    "#73A9CE",
  );
  expect(document.querySelector(".new-project-fixed-selection")).toHaveClass(
    "new-project-fixed-selection--both",
  );
});

test("shows a solid Frame border immediately and sends its canonical values", async () => {
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
  fireEvent.change(
    await screen.findByRole("slider", {
      name: "Espessura da Borda padrão",
    }),
    {
      target: { value: "2500" },
    },
  );
  await user.click(
    screen.getByRole("button", { name: "Usar cor da Borda #C5A46D" }),
  );

  expect(screen.getByText("2.5 mm")).toBeVisible();
  expect(
    screen.getByRole("button", { name: "Usar cor da Borda #C5A46D" }),
  ).toHaveAttribute("aria-pressed", "true");

  const frameBorders = screen.getAllByLabelText(
    /Borda do Frame (esquerdo|direito) [12]/,
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
    .getByLabelText("Borda do Frame esquerdo 1")
    .querySelectorAll("rect");
  expect(firstFrameSegments[0]).toHaveAttribute("x", "12000");
  expect(firstFrameSegments[0]).toHaveAttribute("y", "12000");
  expect(firstFrameSegments[0]).toHaveAttribute("width", "135000");
  expect(firstFrameSegments[0]).toHaveAttribute("height", "2500");
  expect(firstFrameSegments[3]).toHaveAttribute("x", "144500");
  expect(firstFrameSegments[3]).toHaveAttribute("width", "2500");

  await user.click(screen.getByRole("button", { name: "Criar" }));
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
      displayName: "Background esquerdo.jpg",
      previewUrl: "blob:background-left",
    }))
    .mockResolvedValueOnce(selectedDecorative({
      selectionId: "selection-background-right",
      displayName: "Background direito.jpg",
      previewUrl: "blob:background-right",
    }))
    .mockResolvedValueOnce(selectedDecorative({
      selectionId: "selection-overlay-right",
      displayName: "Overlay direito.png",
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
    await screen.findByRole("button", { name: "Lado esquerdo" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de Background" }),
  );
  expect(await screen.findByText("Background esquerdo.jpg")).toBeInTheDocument();
  const leftBackground = screen.getByLabelText("Background do lado esquerdo");
  expect(leftBackground).toHaveAttribute(
    "href",
    "blob:background-left",
  );
  const canonicalBase = screen.getByLabelText("Base branca canônica");
  expect(canonicalBase.parentElement).toBe(leftBackground.parentElement);
  expect(
    [...(canonicalBase.parentElement?.children ?? [])].indexOf(canonicalBase),
  ).toBeLessThan(
    [...(leftBackground.parentElement?.children ?? [])].indexOf(leftBackground),
  );

  await user.click(screen.getByRole("button", { name: "Lado direito" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de Background" }),
  );
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de Overlay" }),
  );
  expect(await screen.findByText("Overlay direito.png")).toBeInTheDocument();
  expect(screen.getByLabelText("Overlay do lado direito")).toHaveAttribute(
    "href",
    "blob:overlay-right",
  );

  await user.click(screen.getByRole("button", { name: "Criar" }));
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
    "Background esquerdo.jpg",
  );
});

test("preserves provisional personalization and releases it when creation is cancelled", async () => {
  const user = userEvent.setup();
  const onCancel = vi.fn();
  const onReleaseDecorative = vi.fn();
  const selection = {
    selectionId: "selection-kept",
    displayName: "Background preservado.jpg",
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
      onReleaseDecorative={onReleaseDecorative}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de Background" }),
  );
  expect(await screen.findByText(selection.displayName)).toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Criar" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de Background" }),
  );
  expect(screen.getByText(selection.displayName)).toBeInTheDocument();
  expect(onReleaseDecorative).not.toHaveBeenCalled();

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
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
  ).toHaveValue("300");
  expect(
    screen.getByRole("textbox", { name: "Quantidade de Lâminas" }),
  ).toHaveValue("18");
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("5");

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(
    await screen.findByRole("button", {
      name: "Usar Background #f7f5f0",
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
  await user.click(
    await screen.findByRole("button", {
      name: "Usar Background #1d2a3a",
    }),
  );
  await user.click(screen.getByRole("button", { name: "Voltar" }));
  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
    { target: { value: "320" } },
  );

  await user.click(
    screen.getByRole("button", {
      name: "Salvar configuração atual como modelo",
    }),
  );
  await user.type(
    screen.getByRole("textbox", { name: "Nome do modelo" }),
    "Estúdio 32 × 30",
  );
  await user.click(screen.getByRole("button", { name: "Salvar" }));
  expect(
    screen.getByRole("combobox", { name: "Modelo inicial" }),
  ).toHaveDisplayValue("Estúdio 32 × 30");

  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
    { target: { value: "300" } },
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Modelo inicial" }),
    "custom-1",
  );
  expect(
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
  ).toHaveValue("320");

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(
    await screen.findByRole("button", {
      name: "Usar Background #1d2a3a",
    }),
  ).toHaveAttribute("aria-pressed", "true");
});

test("shows a typed native picker failure without changing personalization", async () => {
  const user = userEvent.setup();

  render(
    <NewProjectFlow
      onCancel={vi.fn()}
      onChooseDecorative={vi.fn(async () => ({
        status: "failed" as const,
        error: {
          code: "unsupported_image",
          message: "O arquivo escolhido não contém uma imagem JPEG ou PNG.",
          action: "Escolha outro arquivo JPEG ou PNG.",
        },
      }))}
      onCreate={async () => ({ status: "cancelled" })}
      onReleaseDecorative={vi.fn()}
      onValidate={validConfiguration}
    />,
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(
    screen.getByRole("button", { name: "Escolher imagem de Background" }),
  );

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "O arquivo escolhido não contém uma imagem JPEG ou PNG.",
  );
  expect(screen.getByRole("alert")).toHaveTextContent(
    "Escolha outro arquivo JPEG ou PNG.",
  );
  expect(screen.getByText("Cor do Background")).toBeInTheDocument();
});

test("releases a provisional image as soon as it is no longer referenced", async () => {
  const user = userEvent.setup();
  const firstSelection = {
    selectionId: "selection-replaced",
    displayName: "Primeiro Background.jpg",
    previewUrl: "blob:first-background",
  };
  const secondSelection = {
    selectionId: "selection-current",
    displayName: "Background atual.jpg",
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
    name: "Escolher imagem de Background",
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
    name: "Largura da Lâmina fechada",
  });
  const height = screen.getByRole("textbox", {
    name: "Altura da Lâmina fechada",
  });
  await user.click(screen.getByRole("button", { name: "pol" }));
  expect(width).toHaveValue("11.811");
  expect(height).toHaveValue("11.811");
  expect(screen.getByRole("textbox", { name: "Sangria" })).toHaveValue(
    "0.118",
  );
  expect(
    screen.getByRole("textbox", { name: "Área de segurança" }),
  ).toHaveValue("0.197");
  expect(width.closest(".new-project-input-shell")).toHaveTextContent("pol");
  expect(width.closest(".new-project-input-shell")).not.toHaveTextContent(
    "in",
  );
  await user.click(screen.getByRole("button", { name: "mm" }));
  expect(width).toHaveValue("300");
  expect(height).toHaveValue("300");

  await user.click(screen.getByRole("button", { name: "cm" }));
  fireEvent.change(width, { target: { value: "25.4" } });
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

  await user.click(screen.getByRole("button", { name: "Continuar" }));
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
      safetyUm: 5_000,
    },
    structure: {
      sheetCount: 4,
      firstSheet: "singlePage",
      lastSheet: "double",
    },
    visualDefaults: neutralVisualDefaults,
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
    name: "Largura da Lâmina fechada",
  });
  fireEvent.change(width, { target: { value: "60.0001" } });
  expect(screen.queryByText(/micrômetros inteiros/i)).not.toBeInTheDocument();

  await user.click(screen.getByRole("button", { name: "Continuar" }));
  expect(width).toHaveFocus();
  expect(screen.getByText(/micrômetros inteiros/i)).toBeInTheDocument();
  expect(onValidate).not.toHaveBeenCalled();

  fireEvent.change(width, { target: { value: "600" } });
  await waitFor(() => expect(onValidate).toHaveBeenCalledOnce());
  await waitFor(() =>
    expect(screen.queryByText(/micrômetros inteiros/i)).not.toBeInTheDocument(),
  );
  expect(
    screen.getByRole("heading", { name: "Configurações" }),
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
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  expect(await screen.findByText(/altura.*maior que zero/i)).toBeInTheDocument();
  expect(screen.getByText(/DPI inteiro entre 1 e 1\.200/i)).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();
  expect(
    screen.getByRole("textbox", { name: "Altura da Lâmina fechada" }),
  ).toHaveFocus();

  fireEvent.change(
    screen.getByRole("textbox", { name: "Altura da Lâmina fechada" }),
    { target: { value: "250" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));
  await waitFor(() =>
    expect(screen.queryByText(/altura.*maior que zero/i)).not.toBeInTheDocument(),
  );
  expect(screen.queryByText(/DPI inteiro entre/i)).not.toBeInTheDocument();
  expect(
    screen.getByRole("heading", { name: "Configurações" }),
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
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  expect(
    await screen.findByText(/DPI inteiro entre 1 e 1\.200/i),
  ).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();

  fireEvent.change(
    screen.getByRole("textbox", { name: "Altura da Lâmina fechada" }),
    { target: { value: "250" } },
  );
  await waitFor(() => expect(onValidate).toHaveBeenCalledTimes(2));
  expect(screen.getByText(/DPI inteiro entre 1 e 1\.200/i)).toBeInTheDocument();
  expect(screen.getByText(/pelo menos 2 Lâminas/i)).toBeInTheDocument();

  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
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
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await waitFor(() => expect(onValidate).toHaveBeenCalledOnce());
  fireEvent.change(
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
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
    screen.getByRole("heading", { name: "Configurações" }),
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
  await user.click(screen.getByRole("button", { name: "Continuar" }));

  expect(await screen.findByRole("alert")).toHaveTextContent(
    "A validação está indisponível.",
  );
  expect(screen.getByRole("alert")).toHaveTextContent("Tente novamente.");
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
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
    { target: { value: "500" } },
  );
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(await screen.findByRole("button", { name: "Criar" }));
  await act(async () => nativeCreation.resolve({ status: "cancelled" }));

  await user.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
  ).toHaveValue("500");
  await user.click(screen.getByRole("button", { name: "Continuar" }));
  await user.click(await screen.findByRole("button", { name: "Criar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent(
    "Outro objeto passou a ocupar este destino.",
  );
  await user.click(screen.getByRole("button", { name: "Voltar" }));
  expect(
    screen.getByRole("textbox", { name: "Largura da Lâmina fechada" }),
  ).toHaveValue("500");
});
