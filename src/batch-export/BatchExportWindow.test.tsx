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
  phase: "prepared", hasConflicts: false, canContinue: true,
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
  fireEvent.change(screen.getByRole("textbox", { name: "Pasta dos Projetos" }), { target: { value: "C:\\Projetos" } });
  await screen.findByText("1 Projeto encontrado");
  expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  expect(screen.queryByText("Intervalo personalizado")).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Verificar e exportar" }));
  await waitFor(() => expect(api.run).toHaveBeenCalledWith("ask"));
  expect(api.prepare).toHaveBeenCalledWith(ready.options);
  expect(screen.getByRole("textbox", { name: "Pasta dos Projetos" })).toHaveValue("C:\\Projetos");
  expect(screen.queryByText("Pronto para exportar")).not.toBeInTheDocument();
  await act(async () => finish({ ...ready, phase: "finished", items: ready.items.map(item => ({ ...item, status: "completed" })) }));
  expect(await screen.findByText("Exportação concluída")).toBeVisible();
});

test("saved corrections and relinks require explicit Continue even after the last problem disappears", async () => {
  const api = port({ current: async () => ({ ...ready, canContinue: false, items: [{ ...ready.items[0],
    problems: [{ kind: "missingMedia", mediaId: "photo", message: "Imagem ausente: 001.jpg" }] }] }) });
  render(<BatchExportWindow port={api} />);
  fireEvent.click(await screen.findByRole("button", { name: "Religar…" }));
  await screen.findByText("Pronto para exportar");
  expect(api.relink).toHaveBeenCalledWith("album");
  expect(api.run).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Continuar Exportação" }));
  await waitFor(() => expect(api.run).toHaveBeenCalledOnce());
});

test("asks once for generic conflicts with Ignore, Replace or Cancel", async () => {
  const api = port({ current: async () => ({ ...ready, hasConflicts: true }) });
  render(<BatchExportWindow port={api} />);
  fireEvent.click(await screen.findByRole("button", { name: "Continuar Exportação" }));
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
  await screen.findByText("1/4 Álbuns");
  expect(container.querySelector(".ui-owned-window-shell")).toHaveStyle({ width: "400px" });
  expect(screen.getAllByRole("button")).toHaveLength(1);
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(api.cancel).toHaveBeenCalledOnce();
  expect(screen.getByRole("button", { name: "Cancelar" })).toBeDisabled();
});

test("disk full presents a compact pause modal instead of a failed-project table", async () => {
  const api = port({ current: async () => ({ ...ready, phase: "storageFull", canContinue: false }) });
  const { container } = render(<BatchExportWindow port={api} />);
  await screen.findByText("Espaço insuficiente");
  expect(screen.queryByRole("table")).not.toBeInTheDocument();
  expect(container.querySelector(".ui-owned-window-shell")).toHaveStyle({ width: "520px" });
  expect(api.resultReady).toHaveBeenCalled();
  expect(api.run).not.toHaveBeenCalled();
  expect(api.resume).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Retomar" }));
  await screen.findByText("Pronto para exportar");
  expect(api.resume).toHaveBeenCalledWith("batch");
  fireEvent.click(screen.getByRole("button", { name: "Continuar Exportação" }));
  await screen.findByText("Exportação concluída");
  expect(api.run).toHaveBeenCalledOnce();
});

test("the user can cancel a batch paused for disk space without starting another project", async () => {
  const api = port({ current: async () => ({ ...ready, phase: "storageFull", canContinue: false }) });
  render(<BatchExportWindow port={api} />);
  await screen.findByText("Espaço insuficiente");
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  await waitFor(() => expect(api.end).toHaveBeenCalledWith("batch"));
  expect(api.run).not.toHaveBeenCalled();
});
