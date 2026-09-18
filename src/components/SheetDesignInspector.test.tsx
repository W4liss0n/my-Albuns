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
    actions: { disabled: false, onChange: vi.fn(async () => true) },
  };
}

test("equal colors with different origins remain separate and restoration targets the selected role", async () => {
  const input = props("mixed-origin");
  render(<SheetDesignInspector {...input} />);
  const background = within(screen.getByRole("region", { name: "Background" }));
  expect(background.getAllByText("#FFFFFF")).toHaveLength(2);
  expect(background.getByText("Esquerda")).toBeInTheDocument();
  expect(background.getByText("Direita")).toBeInTheDocument();
  expect(background.getByText("Definido nesta lâmina")).toBeInTheDocument();
  expect(background.getByText("Usando o design do álbum")).toBeInTheDocument();
  expect(within(screen.getByRole("region", { name: "Overlay" })).queryByText("Voltar ao design do álbum")).toBeNull();
  await act(async () => fireEvent.click(background.getByRole("button", { name: "Voltar ao design do álbum" })));
  expect(input.actions!.onChange).toHaveBeenCalledExactlyOnceWith(input.sheet.sheetId, "bothSides", { kind: "restoreAlbum", role: "background" });
});

test("scope pointer and keyboard interaction only changes the transient selection", async () => {
  const user = userEvent.setup();
  const input = props("neutral");
  render(<SheetDesignInspector {...input} />);
  const preview = screen.getByRole("group", { name: "Selecionar escopo da Lâmina 01" });
  const left = screen.getByRole("button", { name: "Página esquerda" });
  const both = screen.getByRole("button", { name: "Ambos os lados" });
  await user.hover(left);
  expect(preview).toHaveAttribute("data-hovered-scope", "left");
  expect(both).toHaveAttribute("aria-pressed", "true");
  await user.unhover(left);
  expect(preview).not.toHaveAttribute("data-hovered-scope");
  await user.tab();
  expect(left).toHaveFocus();
  expect(preview).toHaveAttribute("data-hovered-scope", "left");
  await user.keyboard("{Enter}");
  expect(input.onScopeChange).toHaveBeenCalledExactlyOnceWith("left");
  expect(input.actions!.onChange).not.toHaveBeenCalled();
});

test("a color draft commits once, while cancellation, invalid input and a scope change do not edit", async () => {
  const input = props("neutral");
  const view = render(<SheetDesignInspector {...input} scope="left" />);
  const open = () => fireEvent.click(screen.getByRole("button", { name: "Cor do Background da Lâmina" }));
  const field = () => screen.getByRole("textbox", { name: "Cor hexadecimal do Background da Lâmina" });
  open();
  expect(field()).toHaveAttribute("autocomplete", "off");
  fireEvent.change(field(), { target: { value: "#abcdef" } });
  fireEvent.keyDown(field(), { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(input.actions!.onChange).not.toHaveBeenCalled();
  open();
  fireEvent.change(field(), { target: { value: "#oops" } });
  expect(screen.getByRole("button", { name: "Aplicar cor" })).toBeDisabled();
  expect(screen.getByRole("tooltip")).toHaveTextContent("Use uma cor hexadecimal com seis dígitos, como #A1B2C3.");
  expect(field()).toHaveAccessibleDescription("Use uma cor hexadecimal com seis dígitos, como #A1B2C3.");
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
  const input = { ...props("overlay"), actions: { disabled: false, onChange } };
  const view = render(<SheetDesignInspector {...input} scope="left" />);
  fireEvent.click(within(screen.getByRole("region", { name: "Overlay" })).getByRole("button", { name: "Remover" }));
  view.rerender(<SheetDesignInspector {...input} scope="right" />);
  for (const button of screen.getAllByRole("button", { name: "Remover" })) expect(button).toBeDisabled();
  expect(onChange).toHaveBeenCalledExactlyOnceWith(input.sheet.sheetId, "left", { kind: "remove", role: "overlay" });
  await act(async () => finish(success));
  expect(screen.getByRole("button", { name: "Cor do Background da Lâmina" })).toBeEnabled();
  expect(screen.queryByRole("alert")).toBeNull();
});

test("a single-page sheet exposes only its active side and sends its explicit scope", async () => {
  const input = props("single");
  render(<SheetDesignInspector {...input} scope="right" />);
  expect(screen.queryByRole("button", { name: "Página esquerda" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Ambos os lados" })).toBeNull();
  await act(async () => fireEvent.click(within(screen.getByRole("region", { name: "Background" })).getByRole("button", { name: "Remover" })));
  expect(input.actions!.onChange).toHaveBeenCalledExactlyOnceWith(input.sheet.sheetId, "right", { kind: "remove", role: "background" });
});
