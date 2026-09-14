import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import type { GenerationView, ProjectGenerationPort } from "../application/projectGeneration";
import { GenerationWindow } from "./GenerationWindow";

beforeEach(() => vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} unobserve() {} }));
afterEach(() => vi.unstubAllGlobals());
const ready: GenerationView = {
  id: "batch", phase: "prepared", options: { sourceFolder: "C:\\Fotos", destinationFolder: "D:\\Álbuns" }, canContinue: true,
  items: [{ id: "item", name: "001", destination: "D:\\Álbuns\\001.myalbuns", status: "pending", problems: [], conflict: true, canReplace: true, decision: "replace" }],
};
function port(overrides: Partial<ProjectGenerationPort> = {}): ProjectGenerationPort {
  return {
    model: async () => "Modelo", chooseFolder: async () => null, count: async () => 1,
    current: async () => null, prepare: vi.fn(async () => ready), decide: vi.fn(async () => ready), recheck: vi.fn(async () => ready),
    run: vi.fn(async (): Promise<GenerationView> => ({ ...ready, phase: "finished", items: ready.items.map(item => ({ ...item, status: "completed" })) })),
    progress: async () => ({ completed: 1, total: 3 }), onProgress: async () => () => {}, onView: async () => () => {},
    resultReady: vi.fn(async () => {}), cancel: vi.fn(async () => {}), close: vi.fn(async () => {}), ...overrides,
  };
}
test("requires an explicit Continue after the last conflict decision", async () => {
  const api = port({ current: async () => ({ ...ready, canContinue: false, items: [{ ...ready.items[0], decision: null }] }) });
  render(<GenerationWindow port={api} />);
  fireEvent.click(await screen.findByRole("button", { name: "Sobrescrever" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Continuar Geração" })).toBeEnabled());
  expect(api.run).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Continuar Geração" }));
  await screen.findByText("Geração concluída");
  expect(api.run).toHaveBeenCalledOnce();
});

test("uses the standard close control in the configuration header", async () => {
  render(<GenerationWindow port={port()} />);
  await screen.findByText("Modelo");
  expect(screen.getByRole("button", { name: "Fechar janela" })).toBeVisible();
});
test("keeps configuration while the native attempt owns verification and generation", async () => {
  const api = port({ prepare: vi.fn(() => new Promise<GenerationView>(() => {})) });
  render(<GenerationWindow port={api} />);
  await screen.findByText("Modelo");
  fireEvent.change(screen.getByRole("textbox", { name: "Pasta de origem" }), { target: { value: ready.options.sourceFolder } });
  fireEvent.change(screen.getByRole("textbox", { name: "Pasta de destino" }), { target: { value: ready.options.destinationFolder } });
  fireEvent.click(screen.getByRole("button", { name: "Verificar e gerar" }));
  await waitFor(() => expect(api.prepare).toHaveBeenCalledOnce());
  expect(api.run).not.toHaveBeenCalled();
  expect(screen.getByRole("textbox", { name: "Pasta de origem" })).toHaveValue(ready.options.sourceFolder);
  expect(screen.getByRole("button", { name: "Verificar e gerar" })).toBeDisabled();
  expect(screen.queryByText("Preparando geração…")).not.toBeInTheDocument();
});
test("presents pending cancelled items in the final result", async () => {
  render(<GenerationWindow port={port({ current: async () => ({ ...ready, phase: "cancelled" }) })} />);
  await screen.findByText("Geração cancelada");
  expect(screen.getByText("Não gerado")).toBeVisible();
});
test("acknowledges a prepared problem result before the native progress closes", async () => {
  const api = port({ current: async () => ({ ...ready, canContinue: false }) });
  render(<GenerationWindow port={api} />);
  await screen.findByText("Problemas na Geração");
  await waitFor(() => expect(api.resultReady).toHaveBeenCalled());
  expect(api.run).not.toHaveBeenCalled();
});
test("distinguishes ignored projects from failures even when both retain a reason", async () => {
  render(<GenerationWindow port={port({ current: async () => ({ ...ready, phase: "finished", items: [
    { ...ready.items[0], status: "ignored", problems: ["O Projeto está aberto."] },
    { ...ready.items[0], id: "failed", status: "failed", problems: ["A foto mudou."] },
  ] }) })} />);
  await screen.findByText("Ignorado");
  expect(screen.getByText("Falhou")).toBeVisible();
  expect(screen.getByText("O Projeto está aberto.")).toBeVisible();
  expect(screen.getByText("A foto mudou.")).toBeVisible();
});
