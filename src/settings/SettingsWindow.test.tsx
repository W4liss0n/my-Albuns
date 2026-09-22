import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import type { CacheSettingsPort, CacheSettingsStatus } from "../application/cacheSettings";
import { SettingsWindow } from "./SettingsWindow";
import { photoshopSettingsPreview } from "../test/photoshopPreview";

test("Cache cleanup requires confirmation and survives changing tabs while it is pending", async () => {
  let complete!: () => void;
  let clearAllScheduled = false;
  const cachePort: CacheSettingsPort = {
    status: vi.fn(async () => ({ occupiedBytes: 1000, releasableBytes: 500, clearAllScheduled })),
    freeClosedProjects: vi.fn(async () => ({ freedBytes: 500 })),
    clearAll: vi.fn(() => new Promise<{ kind: "scheduled" }>((resolve) => { complete = () => { clearAllScheduled = true; resolve({ kind: "scheduled" }); }; })),
  };
  const close = vi.fn();
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={close} />);
  const clear = screen.getByRole("button", { name: "Limpar prévias" });
  await waitFor(() => expect(clear).toBeEnabled());
  fireEvent.click(clear);
  expect(cachePort.clearAll).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Confirmar" }));
  expect(cachePort.clearAll).toHaveBeenCalledOnce();
  fireEvent.click(screen.getByRole("tab", { name: "Outros" }));
  await act(async () => complete());
  fireEvent.click(screen.getByRole("tab", { name: "Desempenho" }));
  expect(screen.getByRole("status")).toHaveTextContent("quando você abrir o MyAlbuns novamente");
  expect(screen.getByRole("button", { name: "Limpar prévias" })).toBeDisabled();
  expect(cachePort.freeClosedProjects).not.toHaveBeenCalled();
  expect(screen.queryByRole("button", { name: "Confirmar" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Fechar" }));
  expect(close).toHaveBeenCalledOnce();
});

test("canceling cleanup returns to the cache actions without deleting anything", async () => {
  const cachePort: CacheSettingsPort = {
    status: async () => ({ occupiedBytes: 1000, releasableBytes: 500, clearAllScheduled: false }),
    freeClosedProjects: vi.fn(async () => ({ freedBytes: 500 })),
    clearAll: vi.fn(async () => ({ kind: "scheduled" as const })),
  };
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={vi.fn()} />);
  await waitFor(() => expect(screen.getByRole("button", { name: "Limpar prévias" })).toBeEnabled());
  fireEvent.click(screen.getByRole("button", { name: "Limpar prévias" }));
  expect(screen.getByRole("button", { name: "Cancelar" })).toHaveFocus();
  fireEvent.click(screen.getByRole("button", { name: "Cancelar" }));
  expect(screen.getByRole("button", { name: "Limpar prévias" })).toHaveFocus();
  expect(cachePort.freeClosedProjects).not.toHaveBeenCalled();
  expect(cachePort.clearAll).not.toHaveBeenCalled();
});

test.each(["escape", "outside", "focus"])("dismissing the cache confirmation with %s never requests cleanup", async (reason) => {
  const cachePort: CacheSettingsPort = {
    status: async () => ({ occupiedBytes: 1000, releasableBytes: 500, clearAllScheduled: false }),
    freeClosedProjects: vi.fn(async () => ({ freedBytes: 500 })),
    clearAll: vi.fn(async () => ({ kind: "scheduled" as const })),
  };
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={vi.fn()} />);
  const trigger = screen.getByRole("button", { name: "Limpar prévias" });
  await waitFor(() => expect(trigger).toBeEnabled());
  fireEvent.click(trigger);
  expect(screen.getByRole("dialog", { name: "Confirmar limpeza das prévias temporárias" })).toBeInTheDocument();
  expect(screen.getByText("Espaço ocupado")).toBeVisible();
  if (reason === "escape") fireEvent.keyDown(document, { key: "Escape" });
  else if (reason === "outside") fireEvent.pointerDown(screen.getByRole("tab", { name: "Outros" }));
  else fireEvent.focusIn(screen.getByRole("tab", { name: "Outros" }));
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(cachePort.clearAll).not.toHaveBeenCalled();
  expect(cachePort.freeClosedProjects).not.toHaveBeenCalled();
  if (reason === "escape") expect(trigger).toHaveFocus();
});

test("a forwarded settings request changes section and unregisters its listener on close", async () => {
  let request!: (section: "photoshop" | "performance") => void;
  const release = vi.fn();
  const cachePort: CacheSettingsPort = {
    status: async () => ({ occupiedBytes: 0, releasableBytes: 0, clearAllScheduled: false }),
    freeClosedProjects: async () => ({ freedBytes: 0 }), clearAll: async () => ({ kind: "scheduled" }),
  };
  const view = render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={vi.fn()}
    onSectionRequest={async (listener) => { request = listener; return release; }} />);
  await act(async () => request("photoshop"));
  expect(screen.getByRole("tab", { name: "Outros" })).toHaveAttribute("aria-selected", "true");
  view.unmount();
  expect(release).toHaveBeenCalledOnce();
});

test.each(["success", "failure"])("cleanup rejects an older read %s and leaves Photoshop usable while pending", async (outcome) => {
  const initial = { occupiedBytes: 4096, releasableBytes: 4096, clearAllScheduled: false };
  let finishRead!: (status: CacheSettingsStatus) => void;
  let failRead!: (error: Error) => void;
  let finishClear!: (result: { kind: "scheduled" }) => void;
  const cachePort: CacheSettingsPort = {
    status: vi.fn(async () => ({ ...initial, clearAllScheduled: true }))
      .mockResolvedValueOnce(initial)
      .mockImplementationOnce(() => new Promise((resolve, reject) => { finishRead = resolve; failRead = reject; })),
    freeClosedProjects: vi.fn(),
    clearAll: vi.fn(() => new Promise<{ kind: "scheduled" }>((resolve) => { finishClear = resolve; })),
  };
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={vi.fn()} />);
  const trigger = screen.getByRole("button", { name: "Limpar prévias" });
  await waitFor(() => expect(trigger).toBeEnabled());
  fireEvent.focus(window);
  fireEvent.click(trigger);
  const confirm = screen.getByRole("button", { name: "Confirmar" });
  fireEvent.click(confirm);
  fireEvent.click(confirm);
  fireEvent.focus(window);
  expect(cachePort.clearAll).toHaveBeenCalledOnce();
  expect(cachePort.status).toHaveBeenCalledTimes(2);
  fireEvent.click(screen.getByRole("tab", { name: "Outros" }));
  const installations = screen.getByRole("combobox", { name: "Versão utilizada" });
  expect(installations).toBeEnabled();
  fireEvent.change(installations, { target: { value: "2025" } });
  await waitFor(() => expect(installations).toHaveValue("2025"));
  await act(async () => finishClear({ kind: "scheduled" }));
  await act(async () => { if (outcome === "success") finishRead(initial); else failRead(new Error("old read")); });
  fireEvent.click(screen.getByRole("tab", { name: "Desempenho" }));
  expect(screen.getByRole("status")).toHaveTextContent("quando você abrir o MyAlbuns novamente");
  expect(screen.getByRole("button", { name: "Limpar prévias" })).toBeDisabled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
});

test("failed cleanup keeps confirmation available for a successful retry", async () => {
  const initial = { occupiedBytes: 4096, releasableBytes: 4096, clearAllScheduled: false };
  const cachePort: CacheSettingsPort = {
    status: vi.fn(async () => ({ ...initial, occupiedBytes: 2048, releasableBytes: 2048 })).mockResolvedValueOnce(initial),
    freeClosedProjects: vi.fn(),
    clearAll: vi.fn(async () => ({ kind: "cleared" as const, result: { freedBytes: 2048 } })).mockRejectedValueOnce(new Error("cleanup failed")),
  };
  render(<SettingsWindow photoshopPort={photoshopSettingsPreview(null)} cachePort={cachePort} close={vi.fn()} />);
  const trigger = screen.getByRole("button", { name: "Limpar prévias" });
  await waitFor(() => expect(trigger).toBeEnabled());
  fireEvent.click(trigger);
  fireEvent.click(screen.getByRole("button", { name: "Confirmar" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Não foi possível concluir a limpeza");
  expect(screen.getByRole("button", { name: "Confirmar" })).toBeEnabled();
  fireEvent.click(screen.getByRole("button", { name: "Confirmar" }));
  expect(await screen.findByRole("status")).toHaveTextContent("2 KB liberados.");
  expect(cachePort.clearAll).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  expect(trigger).toBeEnabled();
});
