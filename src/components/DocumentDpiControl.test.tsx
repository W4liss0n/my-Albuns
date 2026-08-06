import { act, fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";

import { DocumentDpiControl } from "./DocumentDpiControl";

test("mantém o DPI digitado como draft e aplica uma única alteração consolidada", () => {
  const onApplyDpi = vi.fn();

  render(<DocumentDpiControl dpi={300} onApplyDpi={onApplyDpi} />);

  const input = screen.getByRole("textbox", { name: "DPI" });
  fireEvent.change(input, { target: { value: "600" } });

  expect(input).toHaveValue("600");
  expect(onApplyDpi).not.toHaveBeenCalled();

  fireEvent.click(screen.getByRole("button", { name: "Aplicar DPI" }));

  expect(onApplyDpi).toHaveBeenCalledOnce();
  expect(onApplyDpi).toHaveBeenCalledWith(600);
});

test.each(["", "0", "1201", "300.5", "trezentos"])(
  "recusa o DPI inválido %j antes de solicitar uma alteração",
  (draft) => {
    const onApplyDpi = vi.fn();

    render(<DocumentDpiControl dpi={300} onApplyDpi={onApplyDpi} />);

    const input = screen.getByRole("textbox", { name: "DPI" });
    fireEvent.change(input, { target: { value: draft } });
    fireEvent.submit(input.closest("form")!);

    expect(input).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Informe um DPI inteiro entre 1 e 1.200.",
    );
    expect(onApplyDpi).not.toHaveBeenCalled();
  },
);

test("impede uma segunda solicitação enquanto a alteração aguarda", async () => {
  let finish!: () => void;
  const pending = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const onApplyDpi = vi.fn(() => pending);

  render(<DocumentDpiControl dpi={300} onApplyDpi={onApplyDpi} />);

  const input = screen.getByRole("textbox", { name: "DPI" });
  const form = input.closest("form")!;
  fireEvent.change(input, { target: { value: "600" } });
  fireEvent.submit(form);
  fireEvent.submit(form);

  expect(onApplyDpi).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Aplicar DPI" })).toBeDisabled();

  await act(async () => {
    finish();
    await pending;
  });
});

test("sincroniza o draft com o DPI autoritativo de uma nova projeção", () => {
  const onApplyDpi = vi.fn();
  const view = render(
    <DocumentDpiControl dpi={300} onApplyDpi={onApplyDpi} />,
  );
  const input = screen.getByRole("textbox", { name: "DPI" });

  fireEvent.change(input, { target: { value: "600" } });
  expect(input).toHaveValue("600");

  view.rerender(<DocumentDpiControl dpi={240} onApplyDpi={onApplyDpi} />);

  expect(input).toHaveValue("240");
  expect(screen.getByRole("button", { name: "Aplicar DPI" })).toBeDisabled();
  fireEvent.submit(input.closest("form")!);
  expect(onApplyDpi).not.toHaveBeenCalled();
});
