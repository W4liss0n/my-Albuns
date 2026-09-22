import { act, fireEvent, render, screen, within } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import userEvent from "@testing-library/user-event";
import { decorativeCorpus } from "../test/decorativePreview";
import { SheetDesignInspector } from "./SheetDesignInspector";
import type { ComponentProps } from "react";

function props(name: string): ComponentProps<typeof SheetDesignInspector> {
  const projection = decorativeCorpus.states[name];
  return {
    sheet: projection.composition.sheets[0], visuals: projection.state.album.sheets[0].visuals,
    scope: "both", onScopeChange: vi.fn(), mediaPreviewUrls: {},
    mediaItems: projection.state.album.media,
    actions: { disabled: false, onChange: vi.fn(async () => true), onApplyDecorative: vi.fn(async () => true) },
  };
}

test("equal colors with different origins remain separate and restoration targets the selected role", async () => {
  const input = props("mixed-origin");
  render(<SheetDesignInspector {...input} />);
  const background = within(screen.getByRole("region", { name: "Fundo" }));
  expect(background.getByRole("group", { name: "Opções de fundo" })).toHaveAccessibleDescription(
    "Esquerda: #FFFFFF. Personalizado nesta lâmina. Direita: #FFFFFF. Usando o padrão do álbum.",
  );
  expect(within(screen.getByRole("region", { name: "Sobreposição" })).queryByText("Usar padrão do álbum")).toBeNull();
  await act(async () => fireEvent.click(background.getByRole("button", { name: "Usar padrão do álbum" })));
  expect(input.actions!.onChange).toHaveBeenCalledExactlyOnceWith(input.sheet.sheetId, "bothSides", { kind: "restoreAlbum", role: "background" });
});

test("scope pointer and keyboard interaction only changes the transient selection", async () => {
  const user = userEvent.setup();
  const input = props("neutral");
  render(<SheetDesignInspector {...input} />);
  const preview = screen.getByRole("group", { name: "Aplicar na lâmina 01" });
  const left = screen.getByRole("button", { name: "Página esquerda" });
  const both = screen.getByRole("button", { name: "Ambos os lados" });
  await user.hover(left);
  expect(preview).toHaveAttribute("data-hovered-scope", "left");
  expect(both).toHaveAttribute("aria-pressed", "true");
  await user.unhover(left);
  expect(preview).not.toHaveAttribute("data-hovered-scope");
  const matches = left.matches.bind(left);
  const focusVisible = vi.spyOn(left, "matches").mockImplementation(selector => selector === ":focus-visible" || matches(selector));
  await user.tab();
  expect(left).toHaveFocus();
  expect(preview).toHaveAttribute("data-focused-scope", "left");
  expect(preview).not.toHaveAttribute("data-hovered-scope");
  focusVisible.mockRestore();
  await user.keyboard("{Enter}");
  expect(input.onScopeChange).toHaveBeenCalledExactlyOnceWith("left");
  expect(input.actions!.onChange).not.toHaveBeenCalled();
});

test("a color draft commits once, while cancellation, invalid input and a scope change do not edit", async () => {
  const input = props("neutral");
  const view = render(<SheetDesignInspector {...input} scope="left" />);
  const open = () => fireEvent.click(screen.getByRole("button", { name: "Cor do fundo da lâmina" }));
  const field = () => screen.getByRole("textbox", { name: "Código da cor do fundo da lâmina" });
  open();
  expect(field()).toHaveAttribute("autocomplete", "off");
  fireEvent.change(field(), { target: { value: "#abcdef" } });
  fireEvent.keyDown(field(), { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(input.actions!.onChange).not.toHaveBeenCalled();
  open();
  fireEvent.change(field(), { target: { value: "#oops" } });
  expect(screen.getByRole("button", { name: "Aplicar cor" })).toBeDisabled();
  expect(screen.getByRole("tooltip")).toHaveTextContent("Use uma cor no formato #A1B2C3.");
  expect(field()).toHaveAccessibleDescription("Use uma cor no formato #A1B2C3.");
  view.rerender(<SheetDesignInspector {...input} scope="right" />);
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.queryByRole("tooltip")).toBeNull();
  open();
  fireEvent.change(field(), { target: { value: "#abcdef" } });
  expect(screen.queryByRole("alert")).toBeNull();
  await act(async () => fireEvent.click(screen.getByRole("button", { name: "Aplicar cor" })));
  expect(input.actions!.onChange).toHaveBeenCalledExactlyOnceWith(input.sheet.sheetId, "right", { kind: "backgroundColor", rgb: "#ABCDEF" });
});

test.each([true, false])("a pending removal keeps its original side and blocks repeat actions until completion (%s)", async (success) => {
  let finish!: (value: boolean) => void;
  const onChange = vi.fn(() => new Promise<boolean>((resolve) => { finish = resolve; }));
  const original = props("overlay");
  const input = { ...original, actions: { ...original.actions!, onChange } };
  const view = render(<SheetDesignInspector {...input} scope="left" />);
  fireEvent.click(within(screen.getByRole("region", { name: "Sobreposição" })).getByRole("button", { name: "Sem sobreposição" }));
  view.rerender(<SheetDesignInspector {...input} scope="right" />);
  expect(screen.getByRole("button", { name: "Sem sobreposição" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Remover" })).toBeDisabled();
  expect(onChange).toHaveBeenCalledExactlyOnceWith(input.sheet.sheetId, "left", { kind: "remove", role: "overlay" });
  await act(async () => finish(success));
  expect(screen.getByRole("button", { name: "Cor do fundo da lâmina" })).toBeEnabled();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("a single-page sheet exposes only its active side and sends its explicit scope", async () => {
  const input = props("single");
  render(<SheetDesignInspector {...input} scope="right" />);
  expect(screen.queryByRole("button", { name: "Página esquerda" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Ambos os lados" })).toBeNull();
  await act(async () => fireEvent.click(within(screen.getByRole("region", { name: "Fundo" })).getByRole("button", { name: "Remover" })));
  expect(input.actions!.onChange).toHaveBeenCalledExactlyOnceWith(input.sheet.sheetId, "right", { kind: "remove", role: "background" });
});

test.each([true, false])("decorative selection retains its target and blocks adjacent edits while pending (%s)", async (success) => {
  const input = props("neutral");
  let finish!: (value: boolean) => void;
  const onApplyDecorative = vi.fn(() => new Promise<boolean>(resolve => { finish = resolve; }));
  input.actions = { ...input.actions!, onApplyDecorative };
  const view = render(<SheetDesignInspector {...input} scope="left" />);
  fireEvent.click(screen.getByRole("button", { name: "Escolher decorativo para fundo" }));
  fireEvent.click(screen.getByRole("menuitem", { name: "Usar fundo Textura.png" }));
  expect(onApplyDecorative).toHaveBeenCalledExactlyOnceWith(
    input.sheet.sheetId, "left", "background", "00000000-0000-4000-8000-000000000020",
  );
  view.rerender(<SheetDesignInspector {...input} scope="right" />);
  expect(screen.queryByRole("menu")).toBeNull();
  expect(screen.getByRole("button", { name: "Cor do fundo da lâmina" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Escolher decorativo para sobreposição" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Sem sobreposição" }));
  expect(input.actions.onChange).not.toHaveBeenCalled();
  await act(async () => finish(success));
  expect(screen.getByRole("button", { name: "Cor do fundo da lâmina" })).toBeEnabled();
  expect(screen.queryByRole("menu")).toBeNull();
});

test("one decorative picker is open at a time and changing scope discards the open picker", () => {
  const input = props("neutral");
  const view = render(<SheetDesignInspector {...input} scope="left" />);
  fireEvent.click(screen.getByRole("button", { name: "Escolher decorativo para fundo" }));
  fireEvent.click(screen.getByRole("button", { name: "Escolher decorativo para sobreposição" }));
  expect(screen.getAllByRole("menu")).toHaveLength(1);
  expect(screen.getByRole("menu", { name: "Decorativos para sobreposição" })).toBeInTheDocument();
  view.rerender(<SheetDesignInspector {...input} scope="right" />);
  expect(screen.queryByRole("menu")).toBeNull();
  view.rerender(<SheetDesignInspector {...input} scope="left" />);
  expect(screen.queryByRole("menu")).toBeNull();
  expect(input.actions!.onApplyDecorative).not.toHaveBeenCalled();
});

test("switching directly from decorative selection to color keeps the color editor open", async () => {
  const user = userEvent.setup();
  render(<SheetDesignInspector {...props("neutral")} />);
  await user.click(screen.getByRole("button", { name: "Escolher decorativo para fundo" }));
  await user.click(screen.getByRole("button", { name: "Cor do fundo da lâmina" }));
  expect(screen.queryByRole("menu")).toBeNull();
  expect(screen.getByRole("textbox", { name: "Código da cor do fundo da lâmina" })).toHaveFocus();
});
