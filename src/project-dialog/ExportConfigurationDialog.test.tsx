import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { expect, test, vi } from "vitest";
import { ExportConfigurationDialog } from "./ExportConfigurationDialog";
import type { ProjectDialogState } from "../application/projectDialogPort";

const state: Extract<ProjectDialogState, { kind: "exportConfiguration" }> = {
  kind: "exportConfiguration", busy: false, message: "",
  sheets: [{ sheetId: "opening", number: 1, pageCount: 1 }, { sheetId: "middle", number: 2, pageCount: 2 }, { sheetId: "closing", number: 3, pageCount: 1 }],
  options: { scope: "album", sheetIds: ["opening", "middle", "closing"], mode: "sheet", format: { kind: "jpeg", quality: 100 }, destination: "C:/Álbuns/Teste", overwrite: false },
};

test("exports only the selected continuous sheets and counts active pages for PDF", async () => {
  const user = userEvent.setup(); const onAction = vi.fn();
  render(<ExportConfigurationDialog state={state} onAction={onAction} />);
  expect(screen.getByText("3 arquivos")).toBeInTheDocument();
  await user.click(screen.getByLabelText("Exportar como páginas simples"));
  expect(screen.getByText("4 arquivos")).toBeInTheDocument();
  await user.click(screen.getByLabelText("Intervalo personalizado"));
  fireEvent.change(screen.getByLabelText("Lâminas do intervalo"), { target: { value: "2" } });
  await user.selectOptions(screen.getByLabelText("Formato de exportação"), "pdf");
  expect(screen.getByText("1 PDF · 2 páginas")).toBeInTheDocument();
  expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  await user.click(screen.getByRole("button", { name: "Exportar" }));
  expect(onAction).toHaveBeenCalledWith({ configureExport: { ...state.options, scope: "range", sheetIds: ["middle"], mode: "page", format: { kind: "pdf" } } });
});

test("JPEG quality is local to an opening, restores on double click and is omitted for PNG", async () => {
  const user = userEvent.setup(); const onAction = vi.fn();
  const view = render(<ExportConfigurationDialog state={state} onAction={onAction} />);
  fireEvent.change(screen.getByRole("slider"), { target: { value: "64" } });
  await user.click(screen.getByRole("button", { name: "Exportar" }));
  expect(onAction).toHaveBeenLastCalledWith({ configureExport: { ...state.options, format: { kind: "jpeg", quality: 64 } } });
  fireEvent.doubleClick(screen.getByRole("slider"));
  expect(screen.getByRole("slider")).toHaveValue("100");
  fireEvent.change(screen.getByRole("slider"), { target: { value: "72" } });
  await user.selectOptions(screen.getByLabelText("Formato de exportação"), "png"); await user.click(screen.getByRole("button", { name: "Exportar" }));
  expect(onAction).toHaveBeenLastCalledWith({ configureExport: { ...state.options, format: { kind: "png" } } });
  view.unmount(); render(<ExportConfigurationDialog state={state} onAction={onAction} />);
  expect(screen.getByRole("slider")).toHaveValue("100");
});

test("invalid ranges never submit and contextual export starts at the chosen sheet", async () => {
  const user = userEvent.setup(); const onAction = vi.fn();
  render(<ExportConfigurationDialog state={{ ...state, options: { ...state.options, scope: "range", sheetIds: ["middle"] } }} onAction={onAction} />);
  expect(screen.getByLabelText("Lâminas do intervalo")).toHaveValue("2");
  fireEvent.change(screen.getByLabelText("Lâminas do intervalo"), { target: { value: "3-2" } });
  expect(screen.getByRole("alert")).toBeInTheDocument(); expect(screen.getByRole("button", { name: "Exportar" })).toBeDisabled();
  await user.click(screen.getByRole("button", { name: "Cancelar" })); expect(onAction).toHaveBeenCalledWith("dismissExport");
});

test("contextual export remains an interval even when the album has only one sheet, and Escape closes it", () => {
  const onAction = vi.fn();
  render(<ExportConfigurationDialog state={{ ...state,
    sheets: [{ sheetId: "middle", number: 1, pageCount: 2 }],
    options: { ...state.options, scope: "range", sheetIds: ["middle"] },
  }} onAction={onAction} />);
  expect(screen.getByLabelText("Intervalo personalizado")).toBeChecked();
  fireEvent.keyDown(screen.getByLabelText("Lâminas do intervalo"), { key: "Escape" });
  expect(onAction).toHaveBeenCalledWith("dismissExport");
});

test("wraps keyboard navigation from the destination without leaving the dialog", async () => {
  const user = userEvent.setup();
  render(<><button>Fora do diálogo</button><ExportConfigurationDialog state={{ ...state,
    options: { ...state.options, scope: "range", sheetIds: ["middle"], mode: "page" },
  }} onAction={vi.fn()} /></>);
  const destination = screen.getByLabelText("Pasta de destino");
  expect(destination).toHaveFocus();
  await user.tab({ shift: true });
  expect(screen.getByRole("button", { name: "Exportar" })).toHaveFocus();
  await user.tab();
  expect(destination).toHaveFocus();
  expect(screen.getByLabelText("Exportar como páginas simples")).toBeChecked();
});

test("exports all sheets by default and restores them after selecting a custom interval", async () => {
  const user = userEvent.setup(); const onAction = vi.fn();
  render(<ExportConfigurationDialog state={state} onAction={onAction} />);
  const intervalToggle = screen.getByRole("radio", { name: "Intervalo personalizado" });
  expect(intervalToggle).not.toBeChecked();
  expect(screen.getByRole("radio", { name: "Todas as lâminas" })).toBeChecked();
  expect(screen.getByLabelText("Lâminas do intervalo")).toBeDisabled();
  await user.click(intervalToggle);
  fireEvent.change(screen.getByLabelText("Lâminas do intervalo"), { target: { value: "3-2" } });
  expect(screen.getByRole("button", { name: "Exportar" })).toBeDisabled();
  await user.click(screen.getByRole("radio", { name: "Todas as lâminas" }));
  await user.click(screen.getByRole("button", { name: "Exportar" }));
  expect(onAction).toHaveBeenLastCalledWith({ configureExport: state.options });
});

test.each(["2-3", " 2 – 3 "])("exports a continuous interval entered as %s", async interval => {
  const user = userEvent.setup(); const onAction = vi.fn();
  render(<ExportConfigurationDialog state={{ ...state, options: { ...state.options, scope: "range" } }} onAction={onAction} />);
  fireEvent.change(screen.getByLabelText("Lâminas do intervalo"), { target: { value: interval } });
  await user.click(screen.getByRole("button", { name: "Exportar" }));
  expect(onAction).toHaveBeenCalledWith({ configureExport: { ...state.options, scope: "range", sheetIds: ["middle", "closing"] } });
});

test.each(["", "0", "4", "3-2", "1,3", "1.5", "1-2-3"])("rejects an invalid interval %s", interval => {
  render(<ExportConfigurationDialog state={{ ...state, options: { ...state.options, scope: "range" } }} onAction={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("Lâminas do intervalo"), { target: { value: interval } });
  expect(screen.getByRole("button", { name: "Exportar" })).toBeDisabled();
  expect(screen.getByRole("alert")).toBeInTheDocument();
});
