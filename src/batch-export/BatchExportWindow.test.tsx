import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { BatchExportPort, BatchExportView } from "../application/batchExport";
import { BatchExportWindow } from "./BatchExportWindow";
import { BatchProgressWindow } from "./BatchProgressWindow";

beforeEach(() => vi.stubGlobal("ResizeObserver", class {
  observe() {} disconnect() {} unobserve() {}
}));
afterEach(() => vi.unstubAllGlobals());

const ready: BatchExportView = {
  id: "batch", options: { sourceFolder: "C:\\Projetos", destinationFolder: null, format: { kind: "jpeg", quality: 100 }, mode: "sheet" },
  phase: "prepared", hasConflicts: false, canContinue: true, partialPublication: false,
  items: [{ id: "album", name: "A", projectPath: "C:\\Projetos\\A.myalbuns", destination: "C:\\Projetos\\A", status: "pending", problems: [] }],
};
function port(overrides: Partial<BatchExportPort> = {}): BatchExportPort {
  return {
    current: async () => null, recoveries: async () => [], chooseFolder: async () => "C:\\Projetos", countProjects: async () => 1,
    prepare: vi.fn(async () => ready), recheck: vi.fn(async () => ready), ignore: vi.fn(async () => ready), relink: vi.fn(async () => ready),
    openProject: async () => ({ status: "opened" }), run: vi.fn(async (): Promise<BatchExportView> => ({ ...ready, phase: "finished", items: ready.items.map(item => ({ ...item, status: "completed" })) })),
    cancel: vi.fn(async () => undefined), resume: vi.fn(async () => ready), end: vi.fn(async () => undefined), close: vi.fn(async () => undefined),
    resultReady: vi.fn(async () => undefined), progress: async () => ({ completed: 1, total: 4, percent: 32 }),
    onView: async () => () => undefined, onProgress: async () => () => undefined, ...overrides,
  };
}

test("exports the whole album at maximum JPEG quality without collapsing configuration while progress opens", async () => {
  let finish!: (view: BatchExportView) => void;
  const api = port({ run: vi.fn(() => new Promise<BatchExportView>(resolve => { finish = resolve; })) });
  render(<BatchExportWindow port={api} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Pasta dos projetos" }), { target: { value: "C:\\Projetos" } });
  await screen.findByText("1 projeto encontrado");
  expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  expect(screen.queryByRole("radio", { name: "Intervalo" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Verificar e exportar" }));
  await waitFor(() => expect(api.run).toHaveBeenCalledWith("ask"));
  expect(api.prepare).toHaveBeenCalledWith(ready.options);
  expect(screen.getByRole("textbox", { name: "Pasta dos projetos" })).toHaveValue("C:\\Projetos");
  expect(screen.queryByText("Pronto para exportar")).not.toBeInTheDocument();
  await act(async () => finish({ ...ready, phase: "finished", items: ready.items.map(item => ({ ...item, status: "completed" })) }));
  expect(await screen.findByText("Exportação concluída")).toBeVisible();
});

test("uses the Export dialog's single-pages option for the page mode and an inline alternate folder", async () => {
  const api = port();
  render(<BatchExportWindow port={api} />);
  fireEvent.change(screen.getByRole("textbox", { name: "Pasta dos projetos" }), { target: { value: "C:\\Projetos" } });
  await screen.findByText("1 projeto encontrado");
  expect(screen.queryByRole("combobox", { name: "Modo" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: "Exportar como páginas simples" }));
  const destination = screen.getByRole("textbox", { name: "Pasta de destino" });
  expect(destination).toBeDisabled();
  expect(destination).not.toHaveAttribute("placeholder");
  fireEvent.click(screen.getByRole("radio", { name: "Outra pasta" }));
  expect(screen.getByRole("button", { name: "Verificar e exportar" })).toBeDisabled();
  fireEvent.change(destination, { target: { value: "E:\\Exportações" } });
  fireEvent.click(screen.getByRole("button", { name: "Verificar e exportar" }));
  await waitFor(() => expect(api.prepare).toHaveBeenCalledWith({ ...ready.options, destinationFolder: "E:\\Exportações", mode: "page" }));
});

test("saved corrections and relinks require explicit Continue even after the last problem disappears", async () => {
  const api = port({ current: async () => ({ ...ready, canContinue: false, items: [{ ...ready.items[0],
    problems: [{ kind: "missingMedia", mediaId: "photo", message: "Imagem ausente: 001.jpg", fileName: "001.jpg" }] }] }) });
  render(<BatchExportWindow port={api} />);
  fireEvent.click(await screen.findByRole("button", { name: "Localizar imagens…" }));
  await screen.findByText("Pronto para exportar");
  expect(api.relink).toHaveBeenCalledWith("album");
  expect(api.run).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Continuar exportação" }));
  await waitFor(() => expect(api.run).toHaveBeenCalledOnce());
});

test("lists one entry per project with similar problems grouped and the path on hover", async () => {
  const missing = (fileName: string) => ({ kind: "missingMedia" as const, mediaId: fileName, message: `Imagem ausente: ${fileName}`, fileName });
  const api = port({ current: async () => ({ ...ready, canContinue: false, items: [{ ...ready.items[0], problems: [
    missing("001.jpg"), missing("014.jpg"),
    { kind: "placeholder", mediaId: null, message: "Preencha os quadros vazios e salve o projeto.", fileName: null },
  ] }] }) });
  render(<BatchExportWindow port={api} />);
  const [entry] = await screen.findAllByRole("listitem");
  expect(screen.getAllByRole("listitem")).toHaveLength(1);
  expect(screen.getByText("A")).toHaveAttribute("title", "C:\\Projetos\\A.myalbuns");
  expect(screen.queryByText("C:\\Projetos\\A.myalbuns")).not.toBeInTheDocument();
  expect(entry).toHaveTextContent("2 imagens ausentes: 001.jpg, 014.jpg");
  expect(entry).toHaveTextContent("Preencha os quadros vazios e salve o projeto.");
  expect(screen.queryByText("Imagem ausente: 001.jpg")).not.toBeInTheDocument();
});

test("asks once for generic conflicts with Ignore, Replace or Cancel", async () => {
  const api = port({ current: async () => ({ ...ready, hasConflicts: true }) });
  render(<BatchExportWindow port={api} />);
  fireEvent.click(await screen.findByRole("button", { name: "Continuar exportação" }));
  await screen.findByText("Já existe uma exportação");
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled();
  expect(screen.getByRole("button", { name: "Substituir" })).toBeEnabled();
  expect(api.run).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Ignorar" }));
  await waitFor(() => expect(api.run).toHaveBeenCalledWith("skip"));
});

test("restart offers explicit Resume or End and never resumes automatically", async () => {
  const api = port({ recoveries: async () => [{ id: "batch", sourceFolder: "C:\\Projetos", total: 2, remaining: 1 }] });
  render(<BatchExportWindow port={api} />);
  await screen.findByText("Lote interrompido");
  expect(api.resume).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retomar" }));
  await screen.findByText("Pronto para exportar");
  expect(api.resume).toHaveBeenCalledWith("batch");
  expect(api.run).not.toHaveBeenCalled();
});

test("progress has an independent compact width and only Cancel", async () => {
  const api = port();
  const { container } = render(<BatchProgressWindow port={api} />);
  await screen.findByText("1 álbum de 4");
  expect(container.querySelector(".ui-owned-window-shell")).toHaveStyle({ width: "400px" });
  expect(screen.getAllByRole("button")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(api.cancel).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeDisabled();
});

test("updates the batch album count separately from its overall percentage", async () => {
  let report!: Parameters<BatchExportPort["onProgress"]>[0];
  const api = port({ progress: async () => ({ completed: 0, total: 18, percent: 0 }),
    onProgress: async listener => { report = listener; return () => undefined; } });
  render(<BatchProgressWindow port={api} />);
  await screen.findByText("0 álbuns de 18");
  act(() => report({ completed: 7, total: 18, percent: 43 }));
  expect(screen.getByText("7 álbuns de 18")).toBeVisible();
  expect(screen.getByText("7 álbuns de 18").closest(".ui-progress-dialog__meta"))
    .toBe(screen.getByText("43%").closest(".ui-progress-dialog__meta"));
  expect(screen.queryByRole("status")).not.toBeInTheDocument();
  expect(screen.getByRole("progressbar")).toHaveAttribute("aria-valuenow", "43");
});

test("disk full presents a compact pause modal instead of a failed-project table", async () => {
  const api = port({ current: async () => ({ ...ready, phase: "storageFull", canContinue: false }) });
  const { container } = render(<BatchExportWindow port={api} />);
  await screen.findByText("Espaço insuficiente");
  expect(screen.queryByText(/arquivos do álbum atual já foram exportados/)).not.toBeInTheDocument();
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  expect(container.querySelector(".ui-owned-window-shell")).toHaveStyle({ width: "520px" });
  expect(api.resultReady).toHaveBeenCalled();
  expect(api.run).not.toHaveBeenCalled();
  expect(api.resume).not.toHaveBeenCalled();
  await waitFor(() => expect(screen.getByRole("button", { name: "Retomar" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Retomar" }));
  await screen.findByText("Exportação concluída");
  expect(api.resume).toHaveBeenCalledWith("batch");
  expect(api.run).toHaveBeenCalledOnce();
});

test("the user can cancel a batch paused for disk space without starting another project", async () => {
  const api = port({ current: async () => ({ ...ready, phase: "storageFull", canContinue: false }) });
  render(<BatchExportWindow port={api} />);
  await screen.findByText("Espaço insuficiente");
  await waitFor(() => expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  await waitFor(() => expect(api.end).toHaveBeenCalledWith("batch"));
  expect(api.run).not.toHaveBeenCalled();
});

test("the storage modal discloses a partial publication before the user cancels", async () => {
  const api = port({ current: async () => ({ ...ready, phase: "storageFull", canContinue: false, partialPublication: true }) });
  render(<BatchExportWindow port={api} />);
  await screen.findByText("Espaço insuficiente");
  expect(screen.getByText(/arquivos do álbum atual já foram exportados/)).toBeVisible();
  await waitFor(() => expect(screen.getByRole("button", { name: "Cancelar" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  await waitFor(() => expect(api.end).toHaveBeenCalledWith("batch"));
});

test("cleanup must finish before retrying, and a second full result waits again", async () => {
  let finish!: (freed: boolean) => void;
  const paused: BatchExportView = { ...ready, phase: "storageFull", canContinue: false };
  const storageRecovery = { status: vi.fn(async () => ({ id: "failure", canClearCache: true })),
    clear: vi.fn(() => new Promise<boolean>(resolve => { finish = resolve; })) };
  const api = port({ current: async () => paused, storageRecovery, run: vi.fn(async () => ({ ...paused })) });
  render(<BatchExportWindow port={api} />);
  fireEvent.click(await screen.findByRole("button", { name: "Limpar prévias temporárias e retomar" }));
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeDisabled();
  expect(api.resume).not.toHaveBeenCalled();
  await act(async () => finish(true));
  await waitFor(() => expect(api.run).toHaveBeenCalledOnce());
  await waitFor(() => expect(screen.getByRole("button", { name: "Limpar prévias temporárias e retomar" })).toBeEnabled());
  expect(storageRecovery.clear).toHaveBeenCalledOnce();
  expect(api.resume).toHaveBeenCalledOnce();
});

test("no bytes reclaimed keeps the same modal with manual resume", async () => {
  const api = port({ current: async () => ({ ...ready, phase: "storageFull", canContinue: false }),
    storageRecovery: { status: async () => ({ id: "failure", canClearCache: true }), clear: async () => false } });
  render(<BatchExportWindow port={api} />);
  fireEvent.click(await screen.findByRole("button", { name: "Limpar prévias temporárias e retomar" }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "Limpar prévias temporárias e retomar" })).not.toBeInTheDocument());
  expect(screen.getByRole("button", { name: "Retomar" })).toBeEnabled();
  expect(api.resume).not.toHaveBeenCalled();
});
