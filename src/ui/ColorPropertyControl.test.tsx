import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { ColorPropertyControl } from "./ColorPropertyControl";

test("first click opens the complete palette; hue changes preview and Apply commits once", async () => {
  const user = userEvent.setup();
  const onPreview = vi.fn(), onCommit = vi.fn(), onCancel = vi.fn();
  render(<ColorPropertyControl label="da borda" rgb="#FF0000" {...{ onPreview, onCommit, onCancel }} />);
  await user.click(screen.getByRole("button", { name: "Cor da borda" }));
  const dialog = screen.getByRole("dialog", { name: "Escolher cor da borda" });
  expect(dialog.querySelector(".ui-color-property-area")).toBeVisible();
  expect(dialog.querySelector('input[type="color"]')).toBeNull();
  const hue = screen.getByRole("slider", { name: "Tom da borda" });
  fireEvent.change(hue, { target: { value: "120" } });
  expect(screen.getByRole("textbox")).toHaveValue("#00FF00");
  expect(onPreview).toHaveBeenLastCalledWith("#00FF00");
  expect(onCommit).not.toHaveBeenCalled();
  await user.click(screen.getByRole("button", { name: "Aplicar cor" }));
  expect(onCommit).toHaveBeenCalledExactlyOnceWith("#00FF00");
  expect(screen.getByRole("button", { name: "Cor da borda" })).toHaveFocus();
});

test("mixed selection can cancel or explicitly apply the initial color without ambiguity", async () => {
  const user = userEvent.setup();
  const onCommit = vi.fn(), onCancel = vi.fn();
  const view = render(<ColorPropertyControl label="da borda" rgb={null} onCommit={onCommit} onCancel={onCancel} />);
  const open = () => user.click(screen.getByRole("button", { name: "Cor da borda" }));
  await open();
  await user.keyboard("{Escape}");
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(onCommit).not.toHaveBeenCalled();
  await open();
  await user.click(screen.getByRole("button", { name: "Aplicar cor" }));
  expect(onCommit).toHaveBeenCalledExactlyOnceWith("#000000");
  await open();
  fireEvent.change(screen.getByRole("textbox"), { target: { value: "#abcdef" } });
  view.rerender(<ColorPropertyControl label="da borda" rgb={null} disabled onCommit={onCommit} onCancel={onCancel} />);
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(onCommit).toHaveBeenCalledTimes(1);
  expect(onCancel).toHaveBeenCalled();
});

test("changing hue on white retains it when saturation is subsequently increased", () => {
  render(<ColorPropertyControl label="do fundo" rgb="#FFFFFF" onCommit={vi.fn()} />);
  fireEvent.click(screen.getByRole("button", { name: "Cor do fundo" }));
  fireEvent.change(screen.getByRole("slider", { name: "Tom do fundo" }), { target: { value: "120" } });
  const sliders = screen.getAllByRole("slider");
  fireEvent.change(sliders[0], { target: { value: "100" } });
  expect(screen.getByRole("textbox")).toHaveValue("#00FF00");
});
